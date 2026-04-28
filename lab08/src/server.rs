use std::path::Path;

use clap::Parser;
use env_logger::Env;
use lab08::packet::{TYPE_CMD, TYPE_DATA};
use lab08::rdt::RdtSocket;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;

#[derive(Parser, Debug)]
struct Args {
    /// Host UDP connection to bind with
    #[arg(long, default_value = "127.0.0.1:3000")]
    host: String,

    /// Request timeout
    #[arg(short, long, default_value = "500")]
    timeout: u64,

    /// Packet loss probability
    #[arg(short, long, default_value = "0.3")]
    loss: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("debug"));

    let socket = UdpSocket::bind(&args.host).await?;
    let mut rdt = RdtSocket::new(socket, args.timeout, args.loss);

    log::info!("Server is listening on {}", args.host);

    loop {
        let (p_type, data, addr) = rdt.receive().await?;

        if p_type != TYPE_CMD {
            continue;
        }

        let cmd_str = String::from_utf8_lossy(&data);

        if cmd_str.starts_with("UPLOAD:") {
            let full_name = cmd_str.trim_start_matches("UPLOAD:");

            let safe_name = Path::new(full_name).file_name().unwrap().to_str().unwrap();
            let out_name = format!("data/{}", safe_name);

            log::info!("Client {} uploading '{}'", addr, safe_name);
            let mut file = File::create(&out_name).await.expect("File create failed");

            loop {
                let (_, chunk, _) = rdt.receive().await?;
                if chunk.is_empty() {
                    break;
                }
                file.write_all(&chunk).await?;
            }
            log::info!("File saved as '{}'", out_name);
        } else if cmd_str.starts_with("DOWNLOAD:") {
            let filename = cmd_str.trim_start_matches("DOWNLOAD:");
            log::info!("Client requested '{}'", filename);
            if let Ok(mut file) = File::open(format!("data/{}", filename)).await {
                let mut buf = vec![0u8; 1024];
                loop {
                    let n = file.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    rdt.send(TYPE_DATA, &buf[..n], addr).await?;
                }
                rdt.send(TYPE_DATA, &[], addr).await?;
                log::info!("File '{}' sent", filename);
            } else {
                log::error!("File not found!");
                rdt.send(TYPE_DATA, &[], addr).await?;
            }
        }
    }
}
