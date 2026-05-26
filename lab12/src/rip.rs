use anyhow::Context;
use clap::Parser;
use env_logger::Env;
use serde::{Deserialize, Serialize};
use std::{
    cmp::min, collections::HashMap, fmt::Write, fs::File, io::BufReader, net::SocketAddr, time::{Duration, Instant}
};
use tokio::net::UdpSocket;

const UPDATE_INTERVAL: Duration = Duration::from_secs(2);
const INVALID_TIMEOUT: Duration = Duration::from_secs(6);
const FLUSH_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_DISTANCE: u32 = 16;

#[derive(Serialize, Deserialize, Debug, Clone)]
enum RipMessage {
    Request,
    Response { entries: Vec<RipEntry> },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RipEntry {
    destination: String,
    distance: u32,
}

#[derive(Debug, Clone)]
struct RouteRecord {
    next_hop: String,
    distance: u32,
    last_updated: Instant,
    is_invalid: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct ConnectionConfig {
    from: String,
    to: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct NetworkConfig {
    routers: Vec<String>,
    connections: Vec<ConnectionConfig>,
}

struct Router {
    id: String,
    neighbors: HashMap<String, u32>,
    table: HashMap<String, RouteRecord>,
}

impl Router {
    fn new(id: String, neighbors: HashMap<String, u32>) -> Self {
        let mut table = HashMap::new();
        table.insert(
            id.clone(),
            RouteRecord {
                next_hop: id.clone(),
                distance: 0,
                last_updated: Instant::now(),
                is_invalid: false,
            },
        );
        Router {
            id,
            neighbors,
            table,
        }
    }

    fn update_route(&mut self, dest: &str, next_hop: &str, distance: u32) -> bool {
        let now = Instant::now();
        let mut changed = false;

        match self.table.get_mut(dest) {
            Some(record) => {
                if record.next_hop == next_hop {
                    if record.distance != distance {
                        record.distance = distance;
                        changed = true;
                    }
                    record.last_updated = now;
                    record.is_invalid = distance >= MAX_DISTANCE;
                } else if distance < record.distance {
                    record.next_hop = next_hop.to_string();
                    record.distance = distance;
                    record.last_updated = now;
                    record.is_invalid = distance >= MAX_DISTANCE;
                    changed = true;
                }
            }
            None => {
                if distance < MAX_DISTANCE {
                    self.table.insert(
                        dest.to_string(),
                        RouteRecord {
                            next_hop: next_hop.to_string(),
                            distance,
                            last_updated: now,
                            is_invalid: false,
                        },
                    );
                    changed = true;
                }
            }
        }
        changed
    }

    fn print_table(&self, step: usize) {
        self.print_table_with_label(&format!("Simulation step {}", step));
    }

    fn print_table_with_label(&self, label: &str) {
        let mut output = String::new();
        writeln!(output, "{} of router {} table", label, self.id).unwrap();
        writeln!(
            output,
            "{:<18} {:<20} {:<18} {:<8}",
            "[Source IP]", "[Destination IP]", "[Next Hop]", "[Metric]"
        )
        .unwrap();
        let mut dests: Vec<&String> = self.table.keys().collect();
        dests.sort();
        for dest in dests {
            if let Some(record) = self.table.get(dest) {
                writeln!(
                    output,
                    "{:<18} {:<20} {:<18} {:<8}",
                    self.id, dest, record.next_hop, record.distance
                )
                .unwrap();
            }
        }
        println!("{}", output);
    }
}

async fn run_router_task(id: String, neighbors: HashMap<String, u32>) -> anyhow::Result<Router> {
    let socket = UdpSocket::bind(&id).await?;

    let mut router = Router::new(id.clone(), neighbors);
    let mut update_interval = tokio::time::interval(UPDATE_INTERVAL);
    let mut check_interval = tokio::time::interval(Duration::from_secs(1));
    let mut step_counter = 0;

    let mut last_table_update = Instant::now();

    let mut buf = vec![0u8; 65535];

    if let Err(e) = send_request_message(&socket, &router.neighbors).await {
        log::error!("Router {} error: {}", router.id, e);
    }

    loop {
        tokio::select! {
            recv_res = socket.recv_from(&mut buf) => {
                if let Ok((len, src_addr)) = recv_res {
                    let sender_ip = src_addr.to_string();
                    if let Ok(msg) = serde_json::from_slice::<RipMessage>(&buf[..len]) {
                        match msg {
                            RipMessage::Request => {
                                if let Err(e) = send_response_message(&socket, &sender_ip, &router.table).await {
                                    log::error!("Router {} error: {}", router.id, e);
                                }
                            }
                            RipMessage::Response { entries } => {
                                let mut changed = false;
                                if let Some(&cost) = router.neighbors.get(&sender_ip) {
                                    for entry in entries {
                                        if entry.destination == router.id {
                                            continue;
                                        }
                                        let new_distance = min(entry.distance + cost, MAX_DISTANCE);
                                        if router.update_route(&entry.destination, &sender_ip, new_distance) {
                                            changed = true;
                                        }
                                    }
                                }
                                if changed {
                                    step_counter += 1;
                                    router.print_table(step_counter);
                                    last_table_update = Instant::now();
                                }
                            }
                        }
                    }
                }
            }

            _ = update_interval.tick() => {
                let entries: Vec<RipEntry> = router
                    .table
                    .iter()
                    .map(|(dest, rec)| RipEntry {
                        destination: dest.clone(),
                        distance: rec.distance,
                    })
                    .collect();

                let message = RipMessage::Response { entries };
                if let Ok(data) = serde_json::to_vec(&message) {
                    for neighbor in router.neighbors.keys() {
                        if let Ok(addr) = neighbor.parse::<SocketAddr>() {
                            let _ = socket.send_to(&data, addr).await;
                        }
                    }
                }
            }

            _ = check_interval.tick() => {
                let now = Instant::now();
                if now.duration_since(last_table_update) >= IDLE_TIMEOUT {
                    log::info!("Router {} has been idle for {:?}. Terminating.", id, IDLE_TIMEOUT);
                    return Ok(router);
                }

                let mut changed = false;
                let mut routes_to_remove = Vec::new();

                for (dest, record) in router.table.iter_mut() {
                    if dest == &router.id {
                        continue;
                    }

                    let elapsed = now.duration_since(record.last_updated);

                    if !record.is_invalid && elapsed >= INVALID_TIMEOUT {
                        record.distance = MAX_DISTANCE;
                        record.is_invalid = true;
                        changed = true;
                    }

                    if record.is_invalid && elapsed >= FLUSH_TIMEOUT {
                        routes_to_remove.push(dest.clone());
                    }
                }

                for destination in routes_to_remove {
                    router.table.remove(&destination);
                    changed = true;
                }

                if changed {
                    step_counter += 1;
                    router.print_table(step_counter);
                    last_table_update = Instant::now();
                }
            }
        }
    }
}

async fn send_request_message(
    socket: &UdpSocket,
    neighbors: &HashMap<String, u32>,
) -> anyhow::Result<()> {
    let msg = RipMessage::Request;
    if let Ok(data) = serde_json::to_vec(&msg) {
        for neighbor in neighbors.keys() {
            if let Ok(addr) = neighbor.parse::<SocketAddr>() {
                socket.send_to(&data, addr).await?;
            }
        }
    };
    Ok(())
}

async fn send_response_message(
    socket: &UdpSocket,
    dest: &str,
    table: &HashMap<String, RouteRecord>,
) -> anyhow::Result<()> {
    let entries: Vec<RipEntry> = table
        .iter()
        .map(|(k, v)| RipEntry {
            destination: k.clone(),
            distance: v.distance,
        })
        .collect();

    let msg = RipMessage::Response { entries };
    if let Ok(data) = serde_json::to_vec(&msg) {
        if let Ok(addr) = dest.parse::<SocketAddr>() {
            socket.send_to(&data, addr).await?;
        }
    };
    Ok(())
}

#[derive(Parser, Debug)]
struct Args {
    /// Configuration file
    #[arg(short, long)]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::init_from_env(Env::default().default_filter_or("debug"));

    let file = File::open(&args.config)?;
    let reader = BufReader::new(file);
    let config: NetworkConfig = serde_json::from_reader(reader)
        .with_context(|| format!("Failed to parse JSON config: {}", args.config))?;

    log::info!("Starting simulation using network config: {}", args.config);

    let mut router_neighbors: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for router_id in &config.routers {
        router_neighbors.insert(router_id.clone(), HashMap::new());
    }

    for connection in &config.connections {
        if let Some(n) = router_neighbors.get_mut(&connection.from) {
            n.insert(connection.to.clone(), 1);
        }
        if let Some(n) = router_neighbors.get_mut(&connection.to) {
            n.insert(connection.from.clone(), 1);
        }
    }

    let mut task_handles = Vec::new();

    for router_id in config.routers {
        let neighbors = router_neighbors.remove(&router_id).unwrap_or_default();
        let handle =
            tokio::spawn(async move { run_router_task(router_id.clone(), neighbors).await });
        task_handles.push(handle);

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let mut finished_routers = Vec::new();
    for handle in task_handles {
        match handle.await {
            Ok(Ok(router)) => {
                finished_routers.push(router);
            }
            Ok(Err(e)) => {
                log::error!("Router task finished with error: {:?}", e);
            }
            Err(e) => {
                log::error!("Failed to join router task: {:?}", e);
            }
        }
    }

    finished_routers.sort_by(|a, b| a.id.cmp(&b.id));
    log::info!("Final routing tables:");
    for router in finished_routers {
        router.print_table_with_label("Final state");
    }
    Ok(())
}
