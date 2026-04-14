use std::{fs, path::Path};

use anyhow::Context;
use dialoguer::{Input, theme::ColorfulTheme};
use lettre::{
    Message,
    message::{Mailbox, header::ContentType},
};

pub struct MailConfig {
    sender_name: String,
    sender_email: String,
    receiver_name: String,
    receiver_email: String,
    subject: String,
    file_path_str: String,
}

impl MailConfig {
    pub fn from_user_input() -> anyhow::Result<MailConfig> {        
        let sender_name: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter sender name")
            .interact_text()?;

        let sender_email: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter sender email address")
            .interact_text()?;

        let receiver_name: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter receiver name")
            .interact_text()?;

        let receiver_email: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter receiver email address")
            .interact_text()?;

        let subject: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter email subject")
            .interact_text()?;

        let file_path_str: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter the path to the file you wanna send")
            .interact_text()?;

        Ok(MailConfig {
            sender_name,
            sender_email,
            receiver_name,
            receiver_email,
            subject,
            file_path_str,
        })
    }

    fn read_file(&self) -> anyhow::Result<(ContentType, Vec<u8>)> {
        let path = Path::new(&self.file_path_str);
        let content_type = match path.extension().and_then(|s| s.to_str()) {
            Some("html") | Some("htm") => ContentType::TEXT_HTML,
            Some("txt") => ContentType::TEXT_PLAIN,
            _ => {
                log::warn!("Unknown file extension, defaulting to plain text format");
                ContentType::TEXT_PLAIN
            }
        };
        let file_content =
            fs::read(path).with_context(|| format!("Failed to read file at {}", path.display()))?;
        Ok((content_type, file_content))
    }
}

impl std::convert::From<MailConfig> for anyhow::Result<Message> {
    fn from(value: MailConfig) -> Self {
        let (content_type, content) = value.read_file()?;
        let message = Message::builder()
            .from(Mailbox::new(
                Some(value.sender_name),
                value
                    .sender_email
                    .parse()
                    .context("Invalid sender email format")?,
            ))
            .to(Mailbox::new(
                Some(value.receiver_name),
                value
                    .receiver_email
                    .parse()
                    .context("Invalid receiver email format")?,
            ))
            .subject(value.subject)
            .header(content_type)
            .body(content)?;
        Ok(message)
    }
}
