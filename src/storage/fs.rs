use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use super::prelude::*;
use crate::digest::Digest;

pub struct FileStorage {
    data_dir: PathBuf,
}

impl FileStorage {
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self, CAStorageError> {
        let data_dir = data_dir.into();
        // TODO: check this is a valid dir we can write to
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)?;
        }
        Ok(FileStorage { data_dir })
    }
}

impl CAStorage for FileStorage {
    fn reset(&mut self) -> Result<(), CAStorageError> {
        // TODO: wipe out the entire directory
        Ok(())
    }

    /// look for file by key and only write if not present
    fn set_data(&mut self, data: &[u8]) -> Result<Digest, CAStorageError> {
        let digest = Digest::new_sha_256(data);
        let path = self.data_dir.join(digest.to_string());
        if !path.exists() {
            // Question: do we need file locks?
            std::fs::write(&path, data)?;
        }
        return Ok(digest);
    }

    fn get_data(&self, digest: &Digest) -> Result<Vec<u8>, CAStorageError> {
        let path = self.data_dir.join(digest.to_string());
        if !path.exists() {
            return Err(CAStorageError::NotFound(digest.clone()));
        }

        let mut f = File::open(&path)?;
        let mut data = vec![];
        f.read_to_end(&mut data)?;
        Ok(data)
    }
}
