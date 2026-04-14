mod cli_command;

use std::process::Stdio;

use anyhow::anyhow;
use clap::Parser;
use env_logger::Env;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

use crate::cli_command::CliCommand;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port of the server
    #[arg(default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let server_address = format!("127.0.0.1:{}", args.port);
    log::info!("Listening on {server_address}");
    let listener = TcpListener::bind(&server_address).await?;
    loop {
        let (socket, peer_addr) = listener.accept().await?;
        log::info!("New connection: {}", peer_addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket).await {
                log::error!("Error handling client {}: {:?}", peer_addr, e);
            }
            log::info!("Connection closed: {}", peer_addr);
        });
    }
}

async fn handle_client(mut stream: TcpStream) -> anyhow::Result<()> {
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).await?;
    let parsed_command: CliCommand = match serde_json::from_slice(&buffer[..]) {
        Ok(command) => command,
        Err(e) => {
            stream.write_all(format!("[ERROR] {e}").as_bytes()).await?;
            return Err(anyhow!(e));
        }
    };

    let mut child = match tokio::process::Command::from(parsed_command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            stream.write_all(format!("[ERROR] {e}").as_bytes()).await?;
            return Err(anyhow!(e));
        }
    };

    let Some(stdout) = child.stdout.take() else {
        anyhow::bail!("Failed to capture stdout")
    };
    let Some(stderr) = child.stderr.take() else {
        anyhow::bail!("Failed to capture stderr")
    };
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut stdout_done = false;
    let mut stderr_done = false;

    while !stdout_done || !stderr_done {
        tokio::select! {
            result = stdout_reader.next_line() => {
                match result? {
                    Some(line) => stream.write_all(format!("[STDOUT] {}\n", line).as_bytes()).await?,
                    None => stdout_done = true,
                }
            }
            result = stderr_reader.next_line() => {
                match result? {
                    Some(line) => stream.write_all(format!("[STDERR] {}\n", line).as_bytes()).await?,
                    None => stderr_done = true,
                }
            }
        }
    }
    let status = child.wait().await?;
    stream
        .write_all(format!("[STATUS] {}", status).as_bytes())
        .await?;
    stream.shutdown().await?;
    Ok(())
}
