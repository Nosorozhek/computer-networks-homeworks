use std::net::UdpSocket;

use clap::Parser;
use env_logger::Env;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// IP address of the broadcast
    broadcast_host: String,

    /// Port of the broadcast
    #[arg(default_value_t = 3000)]
    broadcast_port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let broadcast_address = format!("{}:{}", args.broadcast_host, args.broadcast_port);
    let socket = UdpSocket::bind(&broadcast_address)?;

    log::info!("Listening to {}...", broadcast_address);

    let mut buf = [0u8; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buf)?;
        let received_msg = String::from_utf8_lossy(&buf[..len]);
        println!("[{}] {}", addr, received_msg);
    }
}
