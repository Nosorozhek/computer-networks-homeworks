use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

use crate::input::{MailConfig, Message};

pub struct SmtpClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl SmtpClient {
    pub fn connect(address: &str) -> anyhow::Result<Self> {
        let stream = TcpStream::connect(address)?;
        let writer = stream.try_clone()?;
        let mut client = Self {
            reader: BufReader::new(stream),
            writer,
        };

        client.receive_response(220)?;
        Ok(client)
    }

    fn send_command(&mut self, command: &str, expected_code: u16) -> anyhow::Result<String> {
        if command.lines().nth(1).is_some() {
            log::debug!("Sending: {}...", command.lines().next().unwrap_or(""));
        } else {
            log::debug!("Sending: {}", command.lines().next().unwrap_or(""));
        }
        self.writer.write_all(command.as_bytes())?;
        self.writer.write_all(b"\r\n")?;
        self.writer.flush()?;
        self.receive_response(expected_code)
    }

    fn receive_response(&mut self, expected_code: u16) -> anyhow::Result<String> {
        let mut full_response = String::new();
        let code_str = expected_code.to_string();

        loop {
            let mut line = String::new();
            let bytes_read = self.reader.read_line(&mut line)?;

            if bytes_read == 0 {
                return Err(anyhow::anyhow!("Server closed connection"));
            }

            log::debug!("Received: {}", line.trim());
            full_response.push_str(&line);

            if !line.starts_with(&code_str) {
                return Err(anyhow::anyhow!(
                    "Unexpected server response: {}",
                    line.trim()
                ));
            }

            if line.chars().nth(3) == Some(' ') || line.len() == 3 {
                break;
            }

            if line.chars().nth(3) != Some('-') {
                return Err(anyhow::anyhow!(
                    "Failed to parse SMTP response: {}",
                    line.trim()
                ));
            }
        }

        Ok(full_response)
    }

    pub fn send_mail(&mut self, config: MailConfig) -> anyhow::Result<()> {
        self.send_command("EHLO localhost", 250)?;

        self.send_command(&format!("MAIL FROM:<{}>", config.sender_email()), 250)?;
        self.send_command(&format!("RCPT TO:<{}>", config.receiver_email()), 250)?;

        self.send_command("DATA", 354)?;

        let message: Message = Into::<anyhow::Result<Message>>::into(config)?;

        self.send_command(&message.body(), 250)?;

        self.send_command("QUIT", 221)?;

        Ok(())
    }
}
