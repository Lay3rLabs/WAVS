use std::collections::BTreeMap;

use super::prelude::*;
use crate::digest::Digest;

pub struct MemoryStorage {
    data: BTreeMap<Digest, Vec<u8>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        MemoryStorage {
            data: BTreeMap::new(),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        MemoryStorage::new()
    }
}

impl CAStorage for MemoryStorage {
    fn reset(&mut self) -> Result<(), CAStorageError> {
        self.data = BTreeMap::new();
        Ok(())
    }

    fn set_data(&mut self, data: &[u8]) -> Result<Digest, CAStorageError> {
        let digest = Digest::new_sha_256(data);
        if !self.data.contains_key(&digest) {
            self.data.insert(digest.clone(), data.to_vec());
        }
        return Ok(digest);
    }

    fn get_data(&self, digest: &Digest) -> Result<Vec<u8>, CAStorageError> {
        match self.data.get(digest) {
            Some(data) => Ok(data.to_owned()),
            None => Err(CAStorageError::NotFound(digest.clone())),
        }
    }
}
