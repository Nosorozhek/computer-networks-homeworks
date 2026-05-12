use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::{
    signal,
    time::timeout,
    time::{Duration, Instant},
};

use clap::Parser;
use env_logger::Env;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Server host
    host: String,

    /// Number of packets to send
    #[arg(short, long, default_value_t = 0)]
    count: u32,

    /// Delay between ping requests (in seconds)
    #[arg(short, long, default_value_t = 1)]
    delay: u64,
}

struct Stats {
    sent: u32,
    received: u32,
    rtts: Vec<Duration>,
}

const ICMP_ECHO_REPLY: u8 = 0;
const ICMP_DEST_UNREACHABLE: u8 = 3;
const ICMP_SOURCE_QUENCH: u8 = 4;
const ICMP_REDIRECT: u8 = 5;
const ICMP_ECHO_REQUEST: u8 = 8;
const ICMP_TIME_EXCEEDED: u8 = 11;
const ICMP_PARAMETER_PROBLEM: u8 = 12;

fn parse_icmp_error(icmp_type: u8, icmp_code: u8) -> String {
    match icmp_type {
        ICMP_DEST_UNREACHABLE => match icmp_code {
            0 => "Net is unreachable".to_string(),
            1 => "Host is unreachable".to_string(),
            2 => "Protocol is unreachable".to_string(),
            3 => "Port is unreachable".to_string(),
            4 => "Fragmentation is needed and Don't Fragment was set".to_string(),
            5 => "Source route failed".to_string(),
            6 => "Destination network is unknown".to_string(),
            7 => "Destination host is unknown".to_string(),
            _ => format!("Destination Unreachable (Code {})", icmp_code),
        },
        ICMP_SOURCE_QUENCH => "Source quench".to_string(),
        ICMP_REDIRECT => "Redirect".to_string(),
        ICMP_TIME_EXCEEDED => match icmp_code {
            0 => "Time to Live exceeded in transit".to_string(),
            1 => "Fragment reassembly time exceeded".to_string(),
            _ => format!("Time Exceeded (Code {})", icmp_code),
        },
        ICMP_PARAMETER_PROBLEM => "Parameter Problem".to_string(),
        _ => format!("ICMP Type {}, ICMP Code {}", icmp_type, icmp_code),
    }
}

fn checksum(data: &[u8]) -> u16 {
    let mut sum: u64 = 0;
    let mut chunks = data.chunks_exact(2);
    for chunk in &mut chunks {
        let word = ((chunk[0] as u64) << 8) | (chunk[1] as u64);
        sum += word;
    }

    if let Some(&last_byte) = chunks.remainder().first() {
        let word = (last_byte as u64) << 8;
        sum += word;
    }

    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

fn create_icmp_packet(seq: u16, identifier: u16) -> Vec<u8> {
    let mut packet = vec![0u8; 8];
    packet[0] = ICMP_ECHO_REQUEST; // Tyep
    packet[1] = 0; // Code
    packet[4..6].copy_from_slice(&identifier.to_be_bytes());
    packet[6..8].copy_from_slice(&seq.to_be_bytes());

    let payload = b"ping";
    packet.extend_from_slice(payload);

    let checksum = checksum(&packet);
    packet[2..4].copy_from_slice(&checksum.to_be_bytes());
    packet
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let target_addr = format!("{}:0", args.host)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow::anyhow!("Could not resolve host"))?
        .ip();

    log::info!("PING {} ({})", args.host, target_addr);

    let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    // socket.set_ttl(1)?;
    
    let socket = Arc::new(socket);

    let mut stats = Stats {
        sent: 0,
        received: 0,
        rtts: Vec::new(),
    };

    let pid = std::process::id() as u16;
    let mut sequence = 0u16;

    let sig_int = signal::ctrl_c();
    tokio::pin!(sig_int);

    loop {
        sequence += 1;
        stats.sent += 1;

        let packet = create_icmp_packet(sequence, pid);
        let dest = SocketAddr::new(target_addr, 0);
        let start = Instant::now();

        socket.send_to(&packet, &dest.into())?;
        let mut buf = vec![std::mem::MaybeUninit::<u8>::uninit(); 1024];

        let socket_ref = Arc::clone(&socket);

        let task = tokio::task::spawn_blocking(move || {
            socket_ref.recv_from(&mut buf).map(|(n, _)| (n, buf))
        });

        match timeout(Duration::from_secs(1), task).await {
            Ok(Ok(Ok((n, buf)))) => {
                let buf = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, n) };

                let ip_header_len = (buf[0] & 0x0F) as usize * 4;
                if n < ip_header_len + 8 {
                    return Ok(());
                }

                let icmp_type = buf[ip_header_len];
                let icmp_code = buf[ip_header_len + 1];

                match icmp_type {
                    ICMP_ECHO_REPLY => {
                        let received_id =
                            u16::from_be_bytes([buf[ip_header_len + 4], buf[ip_header_len + 5]]);
                        let received_seq =
                            u16::from_be_bytes([buf[ip_header_len + 6], buf[ip_header_len + 7]]);

                        if received_id == pid {
                            let rtt = start.elapsed();
                            log::info!(
                                "{}: icmp_seq={} time={:?}",
                                target_addr, received_seq, rtt
                            );
                            stats.received += 1;
                            stats.rtts.push(rtt);
                        }
                    }
                    ICMP_ECHO_REQUEST => {
                    }
                    _ => {
                        let error_msg = parse_icmp_error(icmp_type, icmp_code);
                        log::warn!("{}: {}", target_addr, error_msg);
                    }
                }
            }
            _ => log::warn!("Request timeout for icmp_seq {}", sequence),
        }

        if args.count > 0 && sequence >= args.count as u16 {
            break;
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(args.delay)) => (),
            _ = &mut sig_int => break,
        }
    }

    print_summary(&args.host, &stats);
    Ok(())
}

fn print_summary(host: &str, stats: &Stats) {
    println!("\n--- {} ping statistics ---", host);
    let loss = if stats.sent > 0 {
        ((stats.sent - stats.received) as f64 / stats.sent as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "{} packets transmitted, {} received, {:.1}% packet loss",
        stats.sent, stats.received, loss
    );

    if !stats.rtts.is_empty() {
        let min = stats.rtts.iter().min().unwrap();
        let max = stats.rtts.iter().max().unwrap();
        let avg = stats.rtts.iter().sum::<Duration>() / stats.received;
        println!("rtt min/avg/max = {:.3?}/{:.3?}/{:.3?}", min, avg, max);
    }
}
