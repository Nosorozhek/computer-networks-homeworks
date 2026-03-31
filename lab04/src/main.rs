mod black_list;
mod connect_request_handler;
mod proxy_service;
mod regular_request_handler;

use clap::Parser;
use env_logger::Env;
use http::{HeaderMap, StatusCode};
use hyper_util::rt::TokioIo;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::net::TcpListener;

use crate::black_list::BlackList;
use crate::proxy_service::*;

type ServerBuilder = hyper::server::conn::http1::Builder;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port of the server
    #[arg(default_value_t = 3000)]
    port: u16,

    /// Directory where cached requests will be stored
    #[arg(short, long)]
    cache_dir: String,

    /// Path to the blacklist file
    #[arg(short, long)]
    blacklist_path: Option<String>,
}

#[derive(Clone)]
struct CachedMetadata {
    expires_at: SystemTime,
    status: StatusCode,
    headers: HeaderMap,
    file_name: String,
}

impl CachedMetadata {
    fn get_cache_filename(uri: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(uri.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn new(uri: &str, expires_at: SystemTime, status: StatusCode, headers: HeaderMap) -> Self {
        Self {
            expires_at,
            status,
            headers,
            file_name: Self::get_cache_filename(uri),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let mut blacklist = BlackList::new();
    if let Some(blacklist_path) = args.blacklist_path {
        blacklist.load_from_file(&blacklist_path)?;
    }
    let shared_blacklist = Arc::new(blacklist);

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = TcpListener::bind(addr).await?;
    log::info!("Listening on http://{}", addr);
    
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let proxy_service = ProxyService::new(shared_blacklist.clone(), args.cache_dir.clone());
        
        tokio::task::spawn(async move {
            if let Err(err) = ServerBuilder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, proxy_service)
                .with_upgrades()
                .await
            {
                log::error!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
