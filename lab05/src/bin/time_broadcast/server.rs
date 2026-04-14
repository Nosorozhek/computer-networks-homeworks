use std::net::SocketAddr;

use chrono::Local;
use clap::Parser;
use env_logger::Env;
use tokio::net::UdpSocket;
use tokio::time::{self, Duration};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Broadcasting IP address
    broadcast_host: String,

    /// Broadcasting port
    #[arg(default_value_t = 3000)]
    broadcast_port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let broadcast_address: SocketAddr =
        format!("{}:{}", args.broadcast_host, &args.broadcast_port).parse()?;
    
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    log::info!("Broadcasting to {}...", &broadcast_address);

    let mut interval = time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;
        let current_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        match socket
            .send_to(current_time.as_bytes(), &broadcast_address)
            .await
        {
            Ok(bytes_sent) => {
                log::debug!("Broadcasted {} bytes: {}", bytes_sent, current_time);
            }
            Err(e) => {
                log::error!("Failed to broadcast: {}", e);
            }
        }
    }
}
