use std::net::{Ipv4Addr, SocketAddr};

use anyhow::anyhow;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpStream, ToSocketAddrs},
};

pub struct FtpClient {
    socket: Connection,
    passive_socket: Option<Connection>,
}

struct Connection {
    socket: BufReader<TcpStream>,
}

struct ResponseCode {
    code: u16,
}

impl Into<ResponseCode> for u16 {
    fn into(self) -> ResponseCode {
        ResponseCode { code: self }
    }
}

impl ResponseCode {
    fn is_ok(&self) -> bool {
        200 <= self.code && self.code < 300
    }

    fn is_err(&self) -> bool {
        400 <= self.code && self.code < 600
    }

    fn is_incomplete(&self) -> bool {
        100 <= self.code && self.code < 200
    }
}

impl Connection {
    pub async fn connect<A>(address: A) -> anyhow::Result<Self>
    where
        A: ToSocketAddrs + Clone,
    {
        Ok(Self {
            socket: BufReader::new(TcpStream::connect(address.clone()).await?),
        })
    }

    async fn send_message(&mut self, command: &str, message: &str) -> anyhow::Result<()> {
        self.socket
            .write_all(format!("{} {}\n", command, message).as_bytes())
            .await?;
        log::debug!("Client: {} {}", command, message);
        Ok(())
    }

    async fn send_secret_message(
        &mut self,
        command: &str,
        message: &str,
        log_message: &str,
    ) -> anyhow::Result<()> {
        self.socket
            .write_all(format!("{} {}\n", command, message).as_bytes())
            .await?;
        log::debug!("Client: {} {}", command, log_message);
        Ok(())
    }

    async fn send_command(&mut self, command: &str) -> anyhow::Result<()> {
        self.socket
            .write_all(format!("{}\n", command).as_bytes())
            .await?;
        log::debug!("Client: {}", command);
        Ok(())
    }

    async fn receive_one(&mut self) -> anyhow::Result<(ResponseCode, String)> {
        log::debug!("Receiving...");
        let mut buf: String = String::new();
        self.socket.read_line(&mut buf).await?;
        log::debug!("Server: {}", buf.trim());
        let line = buf.trim();
        if line.len() < 3 {
            return Err(anyhow!(format!(
                "Failed to parse server response: \"{}\"",
                line
            )));
        }

        let code_str = &line[0..3];
        let message = if line.len() > 4 { &line[3..] } else { "" };

        Ok((
            code_str.parse::<u16>().map_err(|e| anyhow!(e))?.into(),
            message.to_string(),
        ))
    }

    async fn receive_all(&mut self) -> anyhow::Result<(ResponseCode, Vec<String>)> {
        let mut lines: Vec<String> = vec![];
        loop {
            let (code, message) = self.receive_one().await?;
            if code.is_err() {
                return Err(anyhow::anyhow!(message));
            } else {
                lines.push(message.clone());
            }

            if !code.is_incomplete() && !message.starts_with("-") {
                return Ok((code, lines));
            }
        }
    }

    async fn receive_data_string(&mut self) -> anyhow::Result<String> {
        let mut output = String::new();
        self.socket.read_to_string(&mut output).await?;
        log::debug!("Data:\n{}", output.trim());
        Ok(output)
    }

    async fn receive_data_raw(&mut self) -> anyhow::Result<Vec<u8>> {
        let mut output = Vec::<u8>::new();
        self.socket.read_to_end(&mut output).await?;
        log::debug!("Data: received {} bytes of data", output.len());
        Ok(output)
    }

    async fn send_data_raw(&mut self, data: Vec<u8>) -> anyhow::Result<()> {
        self.socket.write(&data).await?;
        log::debug!("Data: sent {} bytes of data", data.len());
        Ok(())
    }
}

impl FtpClient {
    pub async fn connect(address: &str) -> anyhow::Result<Self> {
        let mut conn = Connection::connect(address).await?;
        let (code, _) = conn.receive_all().await?;
        if code.is_err() {
            return Err(anyhow!("Server refused connection"));
        }
        Ok(Self {
            socket: conn,
            passive_socket: None,
        })
    }

    pub async fn send_username(&mut self, username: &str) -> anyhow::Result<()> {
        self.socket.send_message("USER", username).await?;
        let (code, messages) = self.socket.receive_all().await?;
        return if code.is_err() {
            Err(anyhow::anyhow!(messages.join("\n")))
        } else {
            Ok(())
        };
    }

    pub async fn send_password(&mut self, password: &str) -> anyhow::Result<()> {
        self.socket
            .send_secret_message("PASS", password, "********")
            .await?;
        let (code, messages) = self.socket.receive_all().await?;
        return if code.is_err() {
            Err(anyhow::anyhow!(messages.join("\n")))
        } else {
            Ok(())
        };
    }

    pub async fn enter_passive(&mut self) -> anyhow::Result<()> {
        self.socket.send_command("PASV").await?;
        let (code, message) = self.socket.receive_one().await?;
        if !code.is_ok() {
            return Err(anyhow::anyhow!(message));
        }
        let numbers: Vec<u8> = message
            .rsplit_once(")")
            .ok_or(anyhow!(format!(
                "Failed to parse server response: \"{}\"",
                message
            )))?
            .0
            .rsplit_once("(")
            .ok_or(anyhow!(format!(
                "Failed to parse server response: \"{}\"",
                message
            )))?
            .1
            .split(",")
            .map(|s| s.parse::<u8>().map_err(|e| anyhow!(e)))
            .collect::<anyhow::Result<Vec<u8>>>()?;

        if numbers.len() == 6 {
            let ip = Ipv4Addr::new(numbers[0], numbers[1], numbers[2], numbers[3]);
            let port = ((numbers[4] as u16) << 8) | numbers[5] as u16;
            let addr = SocketAddr::new(std::net::IpAddr::V4(ip), port);
            log::debug!("Opening data socket to {}", addr);
            self.passive_socket = Some(Connection::connect(addr).await?);
            Ok(())
        } else {
            Err(anyhow!(format!(
                "Failed to parse server response: \"{}\"",
                message
            )))
        }
    }

    pub async fn list(&mut self) -> anyhow::Result<String> {
        let mut passive = self
            .passive_socket
            .take()
            .ok_or_else(|| anyhow!("Enter passive mode to use LIST command"))?;
        self.socket.send_command("LIST").await?;

        let (code, msg) = self.socket.receive_one().await?;
        if code.is_err() {
            return Err(anyhow!(msg));
        }

        let data = passive.receive_data_string().await?;
        drop(passive);

        let (code, _) = self.socket.receive_all().await?;
        if code.is_err() {
            return Err(anyhow!("List failed"));
        }

        Ok(data)
    }

    pub async fn retrieve(&mut self, filename: &str) -> anyhow::Result<Vec<u8>> {
        let mut passive = self
            .passive_socket
            .take()
            .ok_or_else(|| anyhow!("Enter passive mode to use RETR command"))?;
        self.socket.send_message("RETR", filename).await?;

        let (code, msg) = self.socket.receive_one().await?;
        if code.is_err() {
            return Err(anyhow!(msg));
        }

        let data = passive.receive_data_raw().await?;
        drop(passive);

        let (code, msg) = self.socket.receive_all().await?;
        if code.is_err() {
            return Err(anyhow!(msg.join("\n")));
        }

        Ok(data)
    }

    pub async fn send(&mut self, filename: &str, data: Vec<u8>) -> anyhow::Result<()> {
        let mut passive = self
            .passive_socket
            .take()
            .ok_or_else(|| anyhow!("Enter passive mode to use STOR command"))?;
        self.socket.send_message("STOR", filename).await?;

        let (code, msg) = self.socket.receive_one().await?;
        if code.is_err() {
            return Err(anyhow!(msg));
        }

        passive.send_data_raw(data).await?;
        drop(passive);

        let (code, msg) = self.socket.receive_all().await?;
        if code.is_err() {
            return Err(anyhow!(msg.join("\n")));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete(&mut self, filename: &str) -> anyhow::Result<()> {
        self.socket.send_message("DELE", filename).await?;
        let (code, msg) = self.socket.receive_all().await?;
        if code.is_err() {
            return Err(anyhow!(msg.join("\n")));
        }
        Ok(())
    }

    pub async fn quit(&mut self) -> anyhow::Result<String> {
        self.socket.send_command("QUIT").await?;
        let lines: Vec<String> = vec![];
        loop {
            let (code, message) = self.socket.receive_one().await?;
            if code.is_err() {
                return Err(anyhow::anyhow!(message));
            } else if code.is_ok() {
                break;
            }
        }
        Ok(lines.join("\n"))
    }
}
