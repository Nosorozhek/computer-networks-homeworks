use std::net::SocketAddr;
use std::time::Duration;
use rand::RngExt;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::packet::{Packet, TYPE_ACK, TYPE_CMD};

pub struct RdtSocket {
    socket: UdpSocket,
    timeout_duration: Duration,
    loss_probability: f64,
    seq_send: u8,
    seq_recv: u8,
}

impl RdtSocket {
    pub fn new(socket: UdpSocket, timeout_ms: u64, loss_prob: f64) -> Self {
        Self {
            socket,
            timeout_duration: Duration::from_millis(timeout_ms),
            loss_probability: loss_prob,
            seq_send: 0,
            seq_recv: 0,
        }
    }

    fn should_drop(&self) -> bool {
        let p = self.loss_probability.clamp(0.0, 1.0);
        if p > 0.0 {
            rand::rng().random_bool(p)
        } else {
            false
        }
    }

    pub async fn send(
        &mut self,
        pkt_type: u8,
        payload: &[u8],
        target: SocketAddr,
    ) -> anyhow::Result<()> {
        let pkt = Packet {
            pkt_type,
            seq: self.seq_send,
            payload: payload.to_vec(),
        };
        let bytes = pkt.to_bytes();

        loop {
            if !self.should_drop() {
                log::debug!("SEND: pkt_type={}, seq={}", pkt_type, self.seq_send);
                self.socket.send_to(&bytes, target).await?;
            } else {
                log::debug!("DROP: pkt_type={}, seq={}", pkt_type, self.seq_send);
            }

            let wait_result = timeout(self.timeout_duration, async {
                let mut buf = vec![0u8; 2048];
                loop {
                    if let Ok((len, addr)) = self.socket.recv_from(&mut buf).await {
                        if addr != target { continue; }

                        if let Some(ack_pkt) = Packet::from_bytes(&buf[..len]) {
                            if ack_pkt.pkt_type == TYPE_ACK && ack_pkt.seq == self.seq_send {
                                return true;
                            } else {
                                log::debug!("RCVD: wrong ACK seq={}", ack_pkt.seq);
                            }
                        } else {
                            log::debug!("RCVD: corrupted packet");
                        }
                    }
                }
            }).await;

            match wait_result {
                Ok(true) => {
                    log::debug!("RCVD: ACK seq={}", self.seq_send);
                    self.seq_send = 1 - self.seq_send;
                    break;
                }
                _ => {
                    log::warn!("Timeout: Resending seq={}", self.seq_send);
                }
            }
        }
        Ok(())
    }

    pub async fn receive(&mut self) -> anyhow::Result<(u8, Vec<u8>, SocketAddr)> {
        let mut buf = vec![0u8; 2048];
        loop {
            let (len, addr) = self.socket.recv_from(&mut buf).await?;
            if let Some(pkt) = Packet::from_bytes(&buf[..len]) {
                if pkt.pkt_type == TYPE_ACK { continue; }

                if pkt.pkt_type == TYPE_CMD {
                    self.seq_recv = pkt.seq;
                }

                if pkt.seq == self.seq_recv {
                    log::debug!("RCVD: DATA seq={}", pkt.seq);
                    let ack = Packet {
                        pkt_type: TYPE_ACK,
                        seq: self.seq_recv,
                        payload: vec![],
                    };
                    if !self.should_drop() {
                        self.socket.send_to(&ack.to_bytes(), addr).await?;
                        log::debug!("SEND: ACK seq={}", self.seq_recv);
                    } else {
                        log::debug!("DROP: ACK seq={}", self.seq_recv);
                    }
                    
                    self.seq_recv = 1 - self.seq_recv;
                    return Ok((pkt.pkt_type, pkt.payload, addr));
                } else {
                    log::debug!("RCVD: duplicate data seq={}", pkt.seq);
                    let ack = Packet {
                        pkt_type: TYPE_ACK,
                        seq: pkt.seq,
                        payload: vec![],
                    };
                    if !self.should_drop() {
                        self.socket.send_to(&ack.to_bytes(), addr).await?;
                    }
                }
            } else {
                log::debug!("RCVD: corrupted packet");
            }
        }
    }
}