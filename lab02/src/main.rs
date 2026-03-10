mod app_state;
mod db;
mod error;
mod handlers;
mod icon_manager;
mod models;

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use std::net::SocketAddr;
use std::path::{Path};
use tower_http::trace::TraceLayer;

use clap::Parser;
use env_logger::Env;
use crate::app_state::AppState;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directory where SQLite database is located
    #[arg(short, long)]
    data_dir: String,

    /// Port of the server
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let data_dir_path = Path::new(&args.data_dir);
    if !data_dir_path.exists() {
        anyhow::bail!("Data directory {:?} does not exist", data_dir_path);
    }

    let database_path = data_dir_path.join("products.db");
    let database_url = format!("sqlite:{}", database_path.display());

    let pool = db::init_db(&database_url).await?;
    let icon_manager = icon_manager::IconManager::new(data_dir_path);

    let app = Router::new()
        .route("/product", post(handlers::create_product))
        .route("/product/{id}", get(handlers::get_product))
        .route("/product/{id}", put(handlers::update_product))
        .route("/product/{id}", delete(handlers::delete_product))
        .route("/products", get(handlers::get_products))
        .route("/product/{id}/image", post(handlers::add_icon))
        .route("/product/{id}/image", get(handlers::get_icon))
        .layer(TraceLayer::new_for_http())
        .with_state(AppState { pool: pool.clone() , icon_manager });

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    log::info!("Server started on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
