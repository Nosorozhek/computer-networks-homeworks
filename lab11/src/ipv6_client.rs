use clap::Parser;
use env_logger::Env;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
};

#[derive(Parser, Debug)]
struct Args {
    /// Server address
    #[arg(short, long)]
    server: SocketAddr,

    /// Text to proccess
    #[arg(short, long)]
    text: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("debug"));
    
    log::info!("Raw text: \t \"{}\"", args.text);
    
    let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_only_v6(true)?;
    let socket_addr = SockAddr::from(args.server);
    socket.connect(&socket_addr)?;

    let mut stream: TcpStream = socket.into();
    stream.write_all(args.text.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;

    let mut processed_text: String = String::new();
    stream.read_to_string(&mut processed_text)?;

    log::info!("Processed text: \"{}\"", processed_text);
    Ok(())
}
