use std::{fs, path::Path};

use anyhow::Context;
use base64::{Engine, engine::general_purpose};
use dialoguer::{Input, theme::ColorfulTheme};

pub struct MailConfig {
    sender_name: String,
    sender_email: String,
    receiver_name: String,
    receiver_email: String,
    subject: String,
    file_path_str: String,
}

impl MailConfig {
    pub fn sender_email(&self) -> &str {
        &self.sender_email
    }

    pub fn receiver_email(&self) -> &str {
        &self.receiver_email
    }

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
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let content_type = match ext.to_lowercase().as_str() {
            "html" => ContentType::TextHtml,
            "txt" => ContentType::TextPlain,
            "png" => ContentType::ImagePng,
            "jpg" | "jpeg" => ContentType::ImageJpeg,
            _ => ContentType::Binary,
        };

        let file_content =
            fs::read(path).with_context(|| format!("Failed to read file at {}", path.display()))?;
        Ok((content_type, file_content))
    }
}

impl std::convert::From<MailConfig> for anyhow::Result<Message> {
    fn from(value: MailConfig) -> Self {
        let (content_type, content) = value.read_file()?;

        let is_binary = matches!(
            content_type,
            ContentType::ImagePng | ContentType::ImageJpeg | ContentType::Binary
        );

        let (body, encoding_header) = if is_binary {
            (
                general_purpose::STANDARD.encode(&content),
                "Content-Transfer-Encoding: base64\r\n",
            )
        } else {
            (
                String::from_utf8(content).context("Text file is not valid UTF-8")?,
                "",
            )
        };

        let message = Message {
            message: format!(
                "From: {} <{}>\r\n\
                To: {} <{}>\r\n\
                Subject: {}\r\n\
                Content-Type: {}\r\n\
                {}\
                \r\n\
                {}\r\n\
                .",
                value.sender_name,
                value.sender_email,
                value.receiver_name,
                value.receiver_email,
                value.subject,
                content_type.as_mime_str(),
                encoding_header,
                body
            ),
        };
        Ok(message)
    }
}

pub struct Message {
    message: String,
}

impl Message {
    pub fn body(&self) -> &str {
        &self.message
    }
}

pub enum ContentType {
    TextHtml,
    TextPlain,
    ImagePng,
    ImageJpeg,
    Binary,
}

impl ContentType {
    fn as_mime_str(&self) -> &'static str {
        match self {
            ContentType::TextHtml => "text/html; charset=utf-8",
            ContentType::TextPlain => "text/plain; charset=utf-8",
            ContentType::ImagePng => "image/png",
            ContentType::ImageJpeg => "image/jpeg",
            ContentType::Binary => "application/octet-stream",
        }
    }
}
