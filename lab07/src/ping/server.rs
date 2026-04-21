use clap::Parser;
use env_logger::Env;
use rand::RngExt;
use tokio::net::UdpSocket;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Server host
    #[arg(default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(default_value_t = 3000)]
    port: u16,

    /// Loss ratio
    #[arg(default_value_t = 0.2)]
    loss: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let address = format!("{}:{}", args.host, args.port);
    let socket = UdpSocket::bind(&address).await?;

    log::info!("Listening to {}...", address);

    let mut rng = rand::rng();
    let mut buf = [0; 1024];
    loop {
        let (len, client_address) = socket.recv_from(&mut buf).await?;
        log::info!("{} bytes received from {}", len, client_address);

        
        if rng.random_range(0.0..1.0) > args.loss {
            let len = socket.send_to(&buf[..len], client_address).await?;
            log::info!("{} bytes sent to {}", len, client_address);
        }else {
            log::warn!("Dropped packet from {}", client_address);
        }
    }
}
