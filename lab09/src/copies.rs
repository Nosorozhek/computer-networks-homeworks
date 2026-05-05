use anyhow::Result;
use clap::Parser;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use uuid::Uuid;

static CONFIG: OnceLock<Args> = OnceLock::new();

#[derive(Parser, Debug, Clone)]
struct Args {
    /// Broadcast IP address
    #[arg(short, long, default_value = "255.255.255.255")]
    broadcast_addr: Ipv4Addr,

    /// UDP Port to use
    #[arg(short, long, default_value = "54321")]
    port: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Message {
    sender_id: Uuid,
    address: SocketAddr,
    payload: MessageType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum MessageType {
    Started,
    Alive,
    Leaving,
}

#[derive(Debug, Clone)]
struct PeerInfo {
    addr: SocketAddr,
    last_seen: Instant,
}

struct ShutdownGuard {
    id: Uuid,
    socket: StdUdpSocket,
    target: SocketAddr,
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        let msg = Message {
            sender_id: self.id,
            address: self.socket.local_addr().unwrap(),
            payload: MessageType::Leaving,
        };
        if let Ok(bytes) = serde_json::to_vec(&msg) {
            let _ = self.socket.send_to(&bytes, self.target);
            log::info!("Graceful shutdown: Leaving message sent");
        }
    }
}

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);
const TIMEOUT_INTERVAL: Duration = Duration::from_secs(6);

fn main() {
    let args = Args::parse();
    CONFIG.set(args).expect("Failed to set config");

    env_logger::init();

    dioxus::launch(app);
}

fn process_packet(len: usize, buf: &[u8], my_id: Uuid, peers_map: &Arc<RwLock<HashMap<Uuid, PeerInfo>>>, sock: &Arc<UdpSocket>) {
    if let Ok(msg) = serde_json::from_slice::<Message>(&buf[..len]) {
        if msg.sender_id == my_id {
            return;
        }

        let peers = peers_map.clone();
        let s = sock.clone();
        tokio::spawn(async move {
            let mut map = peers.write().await;
            match msg.payload {
                MessageType::Started | MessageType::Alive => {
                    map.insert(
                        msg.sender_id,
                        PeerInfo {
                            addr: msg.address,
                            last_seen: Instant::now(),
                        },
                    );
                    if matches!(msg.payload, MessageType::Started) {
                        let resp = Message {
                            sender_id: my_id,
                            address: s.local_addr().unwrap(),
                            payload: MessageType::Alive,
                        };
                        let _ = s
                            .send_to(&serde_json::to_vec(&resp).unwrap(), msg.address)
                            .await;
                    }
                }
                MessageType::Leaving => {
                    map.remove(&msg.sender_id);
                }
            }
        });
    }
}

async fn listen_to_clients(
    peers: Arc<RwLock<HashMap<Uuid, PeerInfo>>>,
    local_socket: Arc<UdpSocket>,
    broadcast_socket: Arc<UdpSocket>,
    current_id: Uuid,
) {
    let mut buf_local = [0u8; 1024];
    let mut buf_broadcast = [0u8; 1024];

    loop {
        tokio::select! {
            Ok((len, _)) = local_socket.recv_from(&mut buf_local) => {
                process_packet(len, &buf_local, current_id, &peers, &local_socket);
            }
            Ok((len, _)) = broadcast_socket.recv_from(&mut buf_broadcast) => {
                process_packet(len, &buf_broadcast, current_id, &peers, &local_socket);
            }
        }
    }
}

async fn send_heartbeat(
    peers: Arc<RwLock<HashMap<Uuid, PeerInfo>>>,
    socket: Arc<UdpSocket>,
    target_addr: SocketAddr,
    current_id: Uuid,
) {
    let local_addr = socket.local_addr().unwrap();

    let start_msg = serde_json::to_vec(&Message {
        sender_id: current_id,
        address: local_addr,
        payload: MessageType::Started,
    })
    .unwrap();
    let _ = socket.send_to(&start_msg, target_addr).await;

    loop {
        tokio::time::sleep(HEARTBEAT_INTERVAL).await;
        let message = serde_json::to_vec(&Message {
            sender_id: current_id,
            address: local_addr,
            payload: MessageType::Alive,
        })
        .unwrap();

        let _ = socket.send_to(&message, target_addr).await;

        let mut map = peers.write().await;
        map.retain(|_, info| info.last_seen.elapsed() < TIMEOUT_INTERVAL);
    }
}

fn app() -> Element {
    let current_id = use_memo(|| Uuid::new_v4());
    let mut active_peers = use_signal(|| Vec::<(Uuid, SocketAddr)>::new());

    let config = CONFIG.get().expect("Config not initialized");
    let port = config.port;
    let broadcast_ip = config.broadcast_addr;

    use_future(move || async move {
        let current_id = *current_id.read();
        let peers = Arc::new(RwLock::new(HashMap::<Uuid, PeerInfo>::new()));
        let target_addr = SocketAddr::new(IpAddr::V4(broadcast_ip), port);

        let (raw_local_socket, std_local_socket, broadcast_socket) = setup_sockets(port).await.expect("Failed to setup socket");

        let _shutdown_guard = ShutdownGuard {
            id: current_id,
            socket: std_local_socket,
            target: target_addr,
        };

        let local_socket = Arc::new(raw_local_socket);
        
        let broadcast_socket = Arc::new(broadcast_socket);
        log::info!("Instance ID: {} started on port {}", current_id, port);

        tokio::spawn(listen_to_clients(peers.clone(), local_socket.clone(), broadcast_socket.clone(), current_id));

        tokio::spawn(send_heartbeat(
            peers.clone(),
            local_socket.clone(),
            target_addr,
            current_id,
        ));

        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let mut list: Vec<_> = peers
                .read()
                .await
                .iter()
                .map(|(id, info)| (*id, info.addr))
                .collect::<Vec<_>>();
            list.sort_by_key(|(id, _)| id.as_u128());
            active_peers.set(list);
        }
    });

    rsx! {
        div { style: "padding: 20px; font-family: sans-serif; min-height: 100vh;",
            h1 { "App Copies Counter" }
            div {
                p { "Current ID: ", b { "{current_id}" } }
                p { "Listening on: ", i { "{broadcast_ip}:{port}" } }
                h2 { "Found copies: {active_peers.read().len()}" }

                if active_peers.read().is_empty() {
                    p { style: "color: gray;", "No other copies of this app found..." }
                } else {
                    table { style: "width: 100%;",
                        thead {
                            tr { style: "background: #eee;",
                                th { style: "text-align: left;", "Instance ID" }
                                th { style: "text-align: left;", "Network Address" }
                            }
                        }
                        tbody {
                            for (id, addr) in active_peers.read().iter() {
                                tr { key: "{id}",
                                    td { "{id}" }
                                    td { "{addr}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

async fn setup_sockets(brd_port: u16) -> Result<(UdpSocket, StdUdpSocket, UdpSocket)> {
    let id_sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    id_sock.bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0).into())?;
    id_sock.set_broadcast(true)?;
    let std_id: StdUdpSocket = id_sock.into();
    let std_id_clone = std_id.try_clone()?;
    std_id.set_nonblocking(true)?;
    let tokio_id = UdpSocket::from_std(std_id)?;

    let broadcast_sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    broadcast_sock.set_reuse_address(true)?;
    #[cfg(all(unix))]
    broadcast_sock.set_reuse_port(true)?;
    broadcast_sock.bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), brd_port).into())?;
    let std_broadcast: StdUdpSocket = broadcast_sock.into();
    std_broadcast.set_nonblocking(true)?;
    let tokio_broadcast = UdpSocket::from_std(std_broadcast)?;

    Ok((tokio_id, std_id_clone, tokio_broadcast))
}
