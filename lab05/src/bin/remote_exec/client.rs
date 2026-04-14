mod cli_command;

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

use clap::Parser;
use env_logger::Env;

use crate::cli_command::CliCommand;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// IP address of the server
    server_host: String,

    /// Port of the server
    server_port: u16,

    /// Command to execute remotely
    command: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("warn"));

    let server_address = format!("{}:{}", args.server_host, args.server_port);
    let mut stream = TcpStream::connect(&server_address)?;
    
    let mut parts = args.command.trim().split_whitespace();
    let command_exec = parts.next().unwrap_or(""); 
    let command_args: Vec<&str> = parts.collect();
    let command = CliCommand::new(command_exec, command_args);
    let request: String = serde_json::to_string(&command)?;
    
    log::debug!("Request: {request}");
    stream.write_all(request.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut reader = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    log::info!("Server closed connection.");
                    break;
                }
                Ok(_) => {
                    print!("{line}")
                }
                Err(e) => {
                    log::error!("Error reading: {}", e);
                    break;
                }
            }
        }

    Ok(())
}
