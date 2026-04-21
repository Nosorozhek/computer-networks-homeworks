use std::time::Duration;

use clap::Parser;
use env_logger::Env;
use tokio::{net::UdpSocket, time::{self, sleep}};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Server address
    #[arg(default_value = "127.0.0.1:3000")]
    address: String,
    
    /// Delay between beats in seconds
    #[arg(default_value_t = 1)]
    delay: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    log::info!("Heartbeating to {}...", args.address);

    for id in 0.. {
        let message = format!("{}: {:?}", id, time::Instant::now());
        socket.send_to(message.as_bytes(), &args.address).await?;
        log::info!("{} bytes sent to {}", message.len(), args.address);
        sleep(Duration::from_secs(args.delay)).await;
    }
    Ok(())
}
