use std::{fs, future::Future, path::PathBuf};

use crate::ValkyrieDirectory;
use valkyrie_types::{ValkyrieTypeLegacy, ValkyrieUnionType};

pub enum MaybeFile {
    File(ValkyrieFile),
    Directory(ValkyrieDirectory),
}

pub struct ValkyrieFile {
    _path: PathBuf,
}

pub struct ValkyrieFileHandler {
    _file: tokio::fs::File,
}

pub struct ValkyrieIOError {
    pub message: String,
}

impl ValkyrieFile {
    pub fn exists(&self) -> bool {
        self._path.exists()
    }
    pub async fn delete(&self) -> std::io::Result<()> {
        tokio::fs::remove_file(&self._path).await
    }
    pub async fn open(&self) -> std::io::Result<ValkyrieFileHandler> {
        let file = tokio::fs::File::open(&self._path).await?;
        Ok(ValkyrieFileHandler { _file: file })
    }
    pub async fn read_all_bytes(&mut self) -> std::io::Result<Vec<u8>> {
        return tokio::fs::read(&self._path).await;
    }
    pub fn write_all_bytes(&mut self, buffer: &[u8]) -> std::io::Result<()> {
        fs::write(&self._path, buffer)
    }
}

pub struct UTF8FileHandler {
    _wrap: tokio::fs::File,
}

impl ValkyrieTypeLegacy for ValkyrieFile {
    fn namespace(&self) -> Vec<String> {
        vec!["std".to_string(), "io".to_string()]
    }

    fn type_name(&self) -> String {
        "File".to_string()
    }
}
