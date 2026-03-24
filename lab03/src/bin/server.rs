use clap::Parser;
use env_logger::Env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::{fs, thread};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port of the server
    port: u16,

    /// Maximum number of concurrent connections/tasks
    #[arg(default_value_t = 1)]
    concurrency_level: usize,
}

fn handle_client(mut socket: TcpStream) -> anyhow::Result<()> {
    let mut buffer = [0; 2048];
    let bytes_read = socket.read(&mut buffer)?;

    if bytes_read == 0 {
        return Ok(());
    }

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);

    if let Ok(httparse::Status::Complete(_)) = req.parse(&buffer) {
        let path = req.path.unwrap_or("/");

        let filename = path.trim_start_matches('/');
        log::debug!("Requested file: {}", filename);

        // Delay for demonstration puproses
        sleep(Duration::from_secs(3));

        match fs::read(filename) {
            Ok(contents) => {
                let response_headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    contents.len()
                );
                socket.write_all(response_headers.as_bytes())?;
                socket.write_all(&contents)?;
            }
            Err(_) => {
                let body = "<meta charset='UTF-8'><h1>404 Not Found</h1><p>The requested file was not found ¯\\_(ツ)_/¯.</p>";
                let response = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes())?;
            }
        }

        socket.shutdown(std::net::Shutdown::Write)?;
    } else {
        log::warn!("Request is incomplete or invalid");
    }

    Ok(())
}

struct ConnectionLimit {
    active_connections: Mutex<usize>,
    cv: Condvar,
}

impl ConnectionLimit {
    fn new() -> Self {
        Self {
            active_connections: Mutex::new(0),
            cv: Condvar::new(),
        }
    }

    fn acquire(&self, limit: usize) {
        let mut active = self.active_connections.lock().unwrap();
        active = self
            .cv
            .wait_while(active, |active_connections| *active_connections >= limit)
            .unwrap();

        *active += 1;
    }

    fn release(&self) {
        let mut active = self.active_connections.lock().unwrap();
        *active -= 1;
        self.cv.notify_one();
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    if args.concurrency_level == 0 {
        anyhow::bail!("concurrency_level must be greater than zero");
    }

    let server_address = format!("127.0.0.1:{}", args.port);
    let listener = TcpListener::bind(&server_address)?;

    log::info!(
        "Server started on http://{} with concurrency level {}",
        server_address,
        args.concurrency_level
    );

    let connections_guard = Arc::new(ConnectionLimit::new());

    loop {
        let (socket, peer_addr) = listener.accept()?;
        log::info!("New connection: {}", peer_addr);

        connections_guard.acquire(args.concurrency_level);
        let thread_connections_guard = Arc::clone(&connections_guard);
        thread::spawn(move || {
            if let Err(e) = handle_client(socket) {
                log::error!("Error handling client {}: {:?}", peer_addr, e);
            }
            log::info!("Connection closed: {}", peer_addr);
            thread_connections_guard.release();
        });
    }
}
