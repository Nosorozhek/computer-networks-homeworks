mod input;

use clap::Parser;
use env_logger::Env;
use lettre::{
    Message, SmtpTransport, Transport,
};

use crate::input::MailConfig;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// IP address of the SMTP server
    smtp_address: String,

    /// Port of the SMTP server
    smtp_port: u16,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let mail_config = MailConfig::from_user_input()?;
    let email = Into::<anyhow::Result<Message>>::into(mail_config)?;

    let mailer = SmtpTransport::builder_dangerous(args.smtp_address)
        .port(args.smtp_port)
        .build();

    match mailer.send(&email) {
        Ok(_) => log::info!("Email sent successfully!"),
        Err(e) => log::error!("Could not send email: {e:?}"),
    };
    Ok(())
}
