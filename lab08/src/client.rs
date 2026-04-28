use std::net::SocketAddr;
use std::path::Path;

use clap::{Parser, Subcommand};
use env_logger::Env;
use lab08::packet::{TYPE_CMD, TYPE_DATA};
use lab08::rdt::RdtSocket;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Server address
    #[arg(short, long, default_value = "127.0.0.1:3000")]
    server: String,

    /// Request timeout
    #[arg(short, long, default_value = "500")]
    timeout: u64,

    /// Packet loss probability
    #[arg(short, long, default_value = "0.3")]
    loss: f64,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Send a file to the server
    Send { filename: String },
    /// Receive a file from the server
    Receive { filename: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("debug"));
    
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let mut rdt = RdtSocket::new(socket, args.timeout, args.loss);
    let target: SocketAddr = args.server.parse()?;

    match args.command {
        Commands::Send { filename } => {
            let cmd = format!("UPLOAD:{}", filename);

            rdt.send(TYPE_CMD, cmd.as_bytes(), target).await?;

            let mut file = File::open(&filename).await.expect("File not found");
            let mut buf = vec![0u8; 1024];

            loop {
                let n = file.read(&mut buf).await?;
                if n == 0 {
                    break;
                }

                rdt.send(TYPE_DATA, &buf[..n], target).await?;
            }
            rdt.send(TYPE_DATA, &[], target).await?;
            log::info!("File successfully sent");
        }

        Commands::Receive { filename } => {
            let cmd = format!("DOWNLOAD:{}", filename);
            rdt.send(TYPE_CMD, cmd.as_bytes(), target).await?;

            let filename = Path::new(&filename).file_name().unwrap().to_str().unwrap();
            let mut file = File::create(filename).await?;

            loop {
                let (_, data, _) = rdt.receive().await?;
                if data.is_empty() {
                    break;
                }
                file.write_all(&data).await?;
            }
            log::info!("File successfully received");
        }
    }
    Ok(())
}
