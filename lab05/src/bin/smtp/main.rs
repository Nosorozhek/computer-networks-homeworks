mod client;
mod input;

use anyhow::Context;
use clap::Parser;
use env_logger::Env;

use crate::client::SmtpClient;
use crate::input::MailConfig;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    smtp_host: String,
    smtp_port: u16,
}

fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("debug"));
    let args = Args::parse();

    let mail_config = MailConfig::from_user_input()?;

    log::info!("Connecting to {}:{}...", args.smtp_host, args.smtp_port);
    let address = format!("{}:{}", args.smtp_host, args.smtp_port);

    let mut client = SmtpClient::connect(&address).context("Failed to connect to SMTP server")?;

    match client.send_mail(mail_config) {
        Ok(_) => log::info!("Email sent successfully!"),
        Err(e) => log::error!("Could not send email: {:?}", e),
    }

    Ok(())
}
