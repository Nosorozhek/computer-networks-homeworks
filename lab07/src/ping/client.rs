use std::{sync::Arc, time::Duration};

use tokio::{
    net::UdpSocket,
    sync::Mutex,
    task::JoinSet,
    time::{interval, timeout},
};

use clap::Parser;
use env_logger::Env;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Server host
    address: String,
}
struct PingStats {
    rtts: Vec<Duration>,
    transmitted: u64,
    received: u64,
}

impl PingStats {
    fn new() -> Self {
        Self {
            rtts: vec![],
            transmitted: 0,
            received: 0,
        }
    }
}

async fn ping(ping_stats: Arc<Mutex<PingStats>>, address: String, id: u64) {
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to bind: {}", e);
            return;
        }
    };

    let start = tokio::time::Instant::now();
    let message = b"ping";
    if let Err(e) = socket.send_to(message, &address).await {
        log::error!("{}: Send error: {}", id, e);
        return;
    }

    let mut buf = [0; 1024];
    match timeout(Duration::from_secs(1), socket.recv_from(&mut buf)).await {
        Ok(_) => {
            let duration = tokio::time::Instant::now() - start;
            log::info!("{}: rtt={:.3}", id, duration.as_secs_f32());

            let mut guard = ping_stats.lock().await;
            guard.rtts.push(duration);
            guard.transmitted += 1;
            guard.received += 1;
        }
        Err(_) => {
            log::info!("{}: timeout", id);
            let mut guard = ping_stats.lock().await;
            guard.transmitted += 1;
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let ping_stats: Arc<Mutex<PingStats>> = Arc::new(Mutex::new(PingStats::new()));

    log::info!("PING {}...", &args.address);

    let mut set = JoinSet::new();
    let mut id_counter = 0;
    let mut ping_interval = interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Shutdown signal received. Waiting for active pings to finish...");
                break;
            }

            _ = ping_interval.tick() => {

                set.spawn(ping(
                    ping_stats.clone(),
                    args.address.clone(),
                    id_counter,
                ));
                id_counter += 1;
            }
        }
    }

    while let Some(_) = set.join_next().await {}

    let final_stats = ping_stats.lock().await;
    let received = final_stats.received;
    let transmitted = final_stats.transmitted;
    println!("\n--- Ping {} Statistics ---", args.address);
    println!("Received: {} / Transmitted: {}", received, transmitted);
    if transmitted > 0 {
        let loss = (transmitted - received) as f32 / transmitted as f32 * 100.0;
        println!("Packet loss: ({:.1}%)", loss * 100.0);
    }
    if !final_stats.rtts.is_empty() {
        let min = final_stats.rtts.iter().min().unwrap().as_secs_f32() * 1000.0;
        let max = final_stats.rtts.iter().max().unwrap().as_secs_f32() * 1000.0;
        let sum: Duration = final_stats.rtts.iter().sum();
        let avg = (sum.as_secs_f32() / received as f32) * 1000.0;

        println!("RTT min/avg/max = {:.3}/{:.3}/{:.3} ms", min, avg, max);
    }

    Ok(())
}
