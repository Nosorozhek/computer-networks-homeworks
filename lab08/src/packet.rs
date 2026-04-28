use crate::checksum::{check, checksum};

pub const TYPE_DATA: u8 = 0;
pub const TYPE_ACK: u8 = 1;
pub const TYPE_CMD: u8 = 2;

#[derive(Debug, Clone)]
pub struct Packet {
    pub pkt_type: u8,
    pub seq: u8,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![self.pkt_type, self.seq];
        buf.extend_from_slice(&self.payload);

        let csum = checksum(&buf);
        let mut final_buf = vec![buf[0], buf[1], (csum >> 8) as u8, (csum & 0xFF) as u8];
        final_buf.extend_from_slice(&self.payload);
        final_buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        let pkt_type = bytes[0];
        let seq = bytes[1];
        let csum = ((bytes[2] as u16) << 8) | (bytes[3] as u16);
        let payload = bytes[4..].to_vec();

        let mut verify_buf = vec![bytes[0], bytes[1]];
        verify_buf.extend_from_slice(&payload);

        if !check(&verify_buf, csum) {
            return None;
        }
        Some(Self {
            pkt_type,
            seq,
            payload,
        })
    }
}
