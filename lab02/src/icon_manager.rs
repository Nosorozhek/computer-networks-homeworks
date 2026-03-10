use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct IconManager {
    data_directory: PathBuf,
}

impl IconManager {
    pub fn new(data_directory: &Path) -> Self {
        Self {
            data_directory: data_directory.to_path_buf(),
        }
    }

    fn icon_path(&self, id: i64) -> PathBuf {
        self.data_directory.join(format!("{id}.img"))
    }

    pub fn save_icon(&self, id: i64, data: &[u8]) -> std::io::Result<PathBuf> {
        std::fs::write(self.icon_path(id), data)?;
        Ok(self.icon_path(id))
    }

    pub fn get_icon(&self, id: i64) -> std::io::Result<Vec<u8>> {
        std::fs::read(self.icon_path(id))
    }

    pub fn delete_icon(&self, id: i64) -> std::io::Result<()> {
        let path = self.icon_path(id);
        if path.exists() {
            std::fs::remove_file(path)
        } else {
            Ok(())
        }
    }
}
