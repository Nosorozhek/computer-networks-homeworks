fn compute_sum(data: &[u8]) -> u16 {
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
    
        sum as u16
}

pub fn checksum(data: &[u8]) -> u16 {
    !compute_sum(data) 
}

pub fn check(data: &[u8], checksum_val: u16) -> bool {
    let mut sum = compute_sum(&data) as u64;
        sum += checksum_val as u64;
        
        while (sum >> 16) > 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
        
        sum as u16 == 0xFFFF
}
