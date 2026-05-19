use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::{
        Mutex,
        mpsc::{self, UnboundedReceiver, UnboundedSender},
    },
    time::{Duration, sleep},
};

use env_logger::Env;

#[derive(Clone, Copy, Debug)]
struct DirectionRecord {
    next_router: u64,
    distance: u64,
}

#[derive(Clone, Debug)]
struct Packet {
    destination: u64,
    distance: u64,
}

struct NeighborConnection {
    cost: u64,
    sender: UnboundedSender<Packet>,
}

struct Router {
    id: u64,
    distances: HashMap<u64, DirectionRecord>,
    connections: HashMap<u64, NeighborConnection>,
    neighbor_vectors: HashMap<u64, HashMap<u64, u64>>,
}

impl Router {
    fn new(id: u64) -> Self {
        let mut distances = HashMap::new();
        distances.insert(
            id,
            DirectionRecord {
                next_router: id,
                distance: 0,
            },
        );
        Router {
            id,
            distances,
            connections: HashMap::new(),
            neighbor_vectors: HashMap::new(),
        }
    }

    fn add_connection(&mut self, neighbour_id: u64, cost: u64, sender: UnboundedSender<Packet>) {
        self.connections
            .insert(neighbour_id, NeighborConnection { cost, sender });
        self.neighbor_vectors
            .entry(neighbour_id)
            .or_default()
            .insert(neighbour_id, cost);

        self.update_distance(
            neighbour_id,
            DirectionRecord {
                next_router: neighbour_id,
                distance: cost,
            },
        );

        self.send_full_table_to(neighbour_id);
    }

    fn send_full_table_to(&self, neighbour_id: u64) {
        if let Some(conn) = self.connections.get(&neighbour_id) {
            let sender = conn.sender.clone();
            for (&dest, record) in &self.distances {
                let _ = sender.send(Packet {
                    destination: dest,
                    distance: record.distance,
                });
            }
        }
    }

    fn update_distance(&mut self, destination: u64, new_record: DirectionRecord) {
        let distance_changed = match self.distances.get(&destination) {
            Some(current) => {
                if new_record.distance < current.distance {
                    self.distances.insert(destination, new_record);
                    true
                } else {
                    false
                }
            }
            None => {
                self.distances.insert(destination, new_record);
                true
            }
        };
        if distance_changed {
            self.notify_neighbors(destination, new_record.distance);
        }
    }

    fn notify_neighbors(&self, destination: u64, distance: u64) {
        for (&neighbor_id, conn) in &self.connections {
            if neighbor_id != destination {
                let _ = conn.sender.send(Packet {
                    destination,
                    distance,
                });
            }
        }
    }

    fn process_packet(&mut self, from: u64, packet: Packet) {
        self.neighbor_vectors
            .entry(from)
            .or_default()
            .insert(packet.destination, packet.distance);

        if packet.destination == self.id {
            return;
        }

        let link_cost = match self.connections.get(&from) {
            Some(conn) => conn.cost,
            None => return,
        };
        let new_distance = link_cost.saturating_add(packet.distance);
        self.update_distance(
            packet.destination,
            DirectionRecord {
                next_router: from,
                distance: new_distance,
            },
        );
    }

    fn update_link_cost(&mut self, neighbour: u64, new_cost: u64) {
        if let Some(conn) = self.connections.get_mut(&neighbour) {
            conn.cost = new_cost;
        }

        let updates: Vec<(u64, u64)> = self
            .neighbor_vectors
            .get(&neighbour)
            .map(|vec| {
                vec.iter()
                    .map(|(&dest, &dist)| (dest, new_cost.saturating_add(dist)))
                    .collect()
            })
            .unwrap_or_default();

        for (dest, new_dist) in updates {
            if dest != self.id {
                self.update_distance(
                    dest,
                    DirectionRecord {
                        next_router: neighbour,
                        distance: new_dist,
                    },
                );
            }
        }

        self.update_distance(
            neighbour,
            DirectionRecord {
                next_router: neighbour,
                distance: new_cost,
            },
        );
    }

    fn print_table(&self) {
        let mut destinations: Vec<u64> = self.distances.keys().copied().collect();
        destinations.sort();
        log::info!("Router {} routing table:", self.id);
        for destination in destinations {
            if let Some(rec) = self.distances.get(&destination) {
                log::info!(
                    "{} -> {} -> ... -> {}, cost {}",
                    self.id,
                    rec.next_router,
                    destination,
                    rec.distance,
                );
            }
        }
    }
}

async fn connect(ra: Arc<Mutex<Router>>, rb: Arc<Mutex<Router>>, cost: u64) {
    let (tx_ab, rx_ab) = mpsc::unbounded_channel();
    let (tx_ba, rx_ba) = mpsc::unbounded_channel();

    let mut b = rb.lock().await;
    let mut a = ra.lock().await;
    b.add_connection(a.id, cost, tx_ba);
    a.add_connection(b.id, cost, tx_ab);
    spawn_receiver(rb.clone(), a.id, rx_ab);
    spawn_receiver(ra.clone(), b.id, rx_ba);
}

async fn change_link_cost(ra: Arc<Mutex<Router>>, rb: Arc<Mutex<Router>>, new_cost: u64) {
    let (id_a, id_b) = {
        let a = ra.lock().await;
        let b = rb.lock().await;
        (a.id, b.id)
    };
    if id_a < id_b {
        let mut a = ra.lock().await;
        let mut b = rb.lock().await;
        a.update_link_cost(id_b, new_cost);
        b.update_link_cost(id_a, new_cost);
    } else {
        let mut b = rb.lock().await;
        let mut a = ra.lock().await;
        b.update_link_cost(id_a, new_cost);
        a.update_link_cost(id_b, new_cost);
    }
}

fn spawn_receiver(
    router: Arc<Mutex<Router>>,
    neighbour_id: u64,
    mut receiver: UnboundedReceiver<Packet>,
) {
    tokio::spawn(async move {
        while let Some(packet) = receiver.recv().await {
            let mut r = router.lock().await;
            r.process_packet(neighbour_id, packet);
        }
    });
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let r0 = Arc::new(Mutex::new(Router::new(0)));
    let r1 = Arc::new(Mutex::new(Router::new(1)));
    let r2 = Arc::new(Mutex::new(Router::new(2)));
    let r3 = Arc::new(Mutex::new(Router::new(3)));

    connect(r0.clone(), r1.clone(), 1).await;
    connect(r0.clone(), r2.clone(), 3).await;
    connect(r1.clone(), r2.clone(), 1).await;
    connect(r0.clone(), r3.clone(), 7).await;
    connect(r2.clone(), r3.clone(), 2).await;

    sleep(Duration::from_secs(2)).await;

    log::info!("Initial routing tables:");
    for router in [&r0, &r1, &r2, &r3] {
        router.lock().await.print_table();
    }

    log::info!("Changing cost of link 0 <-> 3 to 2...");
    change_link_cost(r0.clone(), r3.clone(), 2).await;

    sleep(Duration::from_secs(2)).await;

    log::info!("Routing tables after change:");
    for router in [&r0, &r1, &r2, &r3] {
        router.lock().await.print_table();
    }

    Ok(())
}
