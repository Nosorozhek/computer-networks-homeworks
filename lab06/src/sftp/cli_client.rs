mod packet;

use tokio::fs::{read, write};
use anyhow::anyhow;
use dialoguer::{Password, theme::ColorfulTheme};
use env_logger::Env;
use clap::{Parser, Subcommand};

use crate::packet::FtpClient;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Host or IP of the FTP server
    #[arg(long)]
    host: String,

    /// Port of the FTP server
    #[arg(short, long, default_value_t = 21)]
    port: u16,

    /// Username for authentication
    #[arg(short, long)]
    username: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List directories and files on the server
    List,
    /// Upload a local file to the server
    Upload {
        /// Path to the local file
        local_path: String,
        /// Name to save as on the server
        remote_name: String,
    },
    /// Download a file from the server
    Download {
        /// Name of the file on the server
        remote_name: String,
        /// Path to save the file locally
        local_path: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    
    let cli = Cli::parse();

    let password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Enter password for '{}'", cli.username))
        .interact()?;

    let address = format!("{}:{}", cli.host, cli.port);
    log::info!("Connecting to FTP server at {}", address);

    let mut connection = FtpClient::connect(&address).await?;
    connection.send_username(&cli.username).await?;
    connection.send_password(&password).await?;
    log::info!("Successfully logged in.");

    match cli.command {
        Commands::List => {
            log::info!("Fetching directory listing...");
            connection.enter_passive().await?;
            let listing = connection.list().await?;
            println!("\n--- Directory Listing ---\n{}", listing.trim());
        }
        Commands::Upload { local_path, remote_name } => {
            log::info!("Reading local file: {}", local_path);
            let data: Vec<u8> = read(&local_path).await
                .map_err(|e| anyhow!("Failed to read local file: {}", e))?;
            
            log::info!("Uploading as: {}", remote_name);
            connection.enter_passive().await?;
            connection.send(&remote_name, data).await?;
            log::info!("Upload complete.");
        }
        Commands::Download { remote_name, local_path } => {
            log::info!("Downloading file: {}", remote_name);
            connection.enter_passive().await?;
            let data = connection.retrieve(&remote_name).await?;
            
            log::info!("Saving to local file: {}", local_path);
            write(&local_path, data).await
                .map_err(|e| anyhow!("Failed to write local file: {}", e))?;
            log::info!("Download complete.");
        }
    }

    connection.quit().await?;
    Ok(())
}