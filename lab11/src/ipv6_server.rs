use clap::Parser;
use env_logger::Env;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port of the server
    #[arg(default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("debug"));

    let listener = TcpListener::bind(format!("[::1]:{}", args.port)).await?;
    log::info!("Started listening to: {}", listener.local_addr()?);
    
    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut processed_text: String = String::new();
            if let Err(e) = stream.read_to_string(&mut processed_text).await {
                log::warn!("{}", e)
            };
            log::debug!("Received: {} bytes", processed_text.len());
            processed_text = processed_text.to_uppercase();
            if let Err(e) = stream.write(processed_text.as_bytes()).await {
                log::warn!("{}", e)
            };
        });
    }
}
