use clap::Parser;
use std::net::{IpAddr, TcpListener};

#[derive(Parser, Debug)]
struct Args {
    /// IP address
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: IpAddr,

    /// Start port of the range
    #[arg(short, long)]
    start_port: u16,

    /// End port of the range
    #[arg(short, long)]
    end_port: u16,
}

fn main() {
    let args = Args::parse();

    if args.start_port > args.end_port {
        eprintln!("Error: start port exceeds end port");
        return;
    }

    println!("IP:    {}", args.ip);
    println!("Range: {} - {}", args.start_port, args.end_port);

    let mut found_any = false;

    for port in args.start_port..=args.end_port {
        match TcpListener::bind((args.ip, port)) {
            Ok(_) => {
                println!("Port {} is available", port);
                found_any = true;
            }
            Err(_) => {
                continue;
            }
        }
    }

    if !found_any {
        println!("No available ports in the specified range");
    }
}