use std::collections::HashSet;

pub struct BlackList {
    addresses: HashSet<String>,
}

impl Default for BlackList {
    fn default() -> Self {
        Self {
            addresses: Default::default(),
        }
    }
}

#[allow(dead_code)]
impl BlackList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_from_file(&mut self, blacklist_path: &str) -> anyhow::Result<()> {
        if let Ok(file_content) = std::fs::read_to_string(blacklist_path) {
            for line in file_content.lines() {
                let domain = line.trim();
                if !domain.is_empty() {
                    self.add_address(domain);
                }
            }
            Ok(())
        } else {
            anyhow::bail!("Failed to open the blacklist file")
        }
    }

    pub fn contains_address(&self, address: &str) -> bool {
        self.addresses
            .iter()
            .any(|banned| address.ends_with(banned))
    }

    pub fn add_address(&mut self, address: &str) {
        self.addresses.insert(address.to_owned());
    }
}
