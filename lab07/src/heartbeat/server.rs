use std::{collections::HashMap, sync::Arc, time::Duration};

use clap::Parser;
use env_logger::Env;
use tokio::{
    net::UdpSocket,
    sync::Mutex,
    time::{self, Instant},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(default_value_t = 3000)]
    port: u16,

    /// Seconds to wait before considering a client disconnected
    #[arg(default_value_t = 3)]
    timeout: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", args.port)).await?);

    let clients_last_seens: Arc<Mutex<HashMap<String, Instant>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let timeout_duration = Duration::from_secs(args.timeout);

    log::info!("Heartbeat server listening on port {}...", args.port);
    log::info!("Timeout set to {} seconds", args.timeout);

    let clients_cleanup = Arc::clone(&clients_last_seens);
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let mut map = clients_cleanup.lock().await;
            let now = Instant::now();

            map.retain(|addr, last_seen| {
                if now.duration_since(*last_seen) > timeout_duration {
                    log::warn!("[!] Client Disconnected: {}", addr);
                    false
                } else {
                    true
                }
            });
        }
    });

    let mut buf = [0; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let message = String::from_utf8_lossy(&buf[..len]);

        let mut map = clients_last_seens.lock().await;

        if !map.contains_key(&addr.to_string()) {
            log::info!("[+] New Client Connected: {}", addr);
        }

        map.insert(addr.to_string(), Instant::now());

        log::debug!("Heartbeat from {}: {}", addr, message);
    }
}
