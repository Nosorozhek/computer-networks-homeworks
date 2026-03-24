use std::{
    io::{Read, Write},
    net::TcpStream,
};

use clap::Parser;
use env_logger::Env;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// IP address of the server
    server_host: String,

    /// Port of the server
    server_port: u16,

    /// Path to the requested file
    filename: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let server_address = format!("{}:{}", args.server_host, args.server_port);
    let mut stream = TcpStream::connect(&server_address)?;

    let filename_uri = if args.filename.starts_with('/') {
        args.filename.clone()
    } else {
        format!("/{}", args.filename)
    };

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        filename_uri, server_address
    );

    stream.write_all(request.as_bytes())?;

    let mut response: Vec<u8> = vec![];
    stream.read_to_end(&mut response)?;

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut res = httparse::Response::new(&mut headers);

    if let Ok(httparse::Status::Complete(offset)) = res.parse(&response) {
        let body = &response[offset..];

        println!("HTTP Version: 1.{}", res.version.unwrap_or(1));
        println!("Status Code: {} {}", res.code.unwrap_or(0), res.reason.unwrap_or(""));
        println!("Headers:");
        for header in res.headers {
            println!(
                "\t{}: {}",
                header.name,
                String::from_utf8_lossy(header.value)
            );
        }

        println!("\nBody:");
        println!("{}", String::from_utf8_lossy(body));
    } else {
        anyhow::bail!("response is incomplete or invalid!")
    }

    Ok(())
}
