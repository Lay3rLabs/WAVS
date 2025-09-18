use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use tracing::instrument;
use wavs_types::AnyDigest;

use super::prelude::*;

#[derive(Clone)]
pub struct MemoryStorage {
    data: Arc<RwLock<BTreeMap<AnyDigest, Vec<u8>>>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        MemoryStorage {
            data: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        MemoryStorage::new()
    }
}

impl CAStorage for MemoryStorage {
    #[instrument(skip(self), fields(subsys = "CaStorage"))]
    fn reset(&self) -> Result<(), CAStorageError> {
        let mut tree = self.data.write()?;
        tree.clear();
        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "CaStorage"))]
    fn set_data(&self, data: &[u8]) -> Result<AnyDigest, CAStorageError> {
        let digest = AnyDigest::hash(data);
        let mut tree = self.data.write()?;
        if !tree.contains_key(&digest) {
            tree.insert(digest.clone(), data.to_vec());
        }
        Ok(digest)
    }

    #[instrument(skip(self), fields(subsys = "CaStorage"))]
    fn get_data(&self, digest: &AnyDigest) -> Result<Vec<u8>, CAStorageError> {
        let tree = self.data.read()?;
        match tree.get(digest) {
            Some(data) => Ok(data.to_owned()),
            None => Err(CAStorageError::NotFound(digest.clone())),
        }
    }

    #[instrument(skip(self), fields(subsys = "CaStorage"))]
    fn data_exists(&self, digest: &AnyDigest) -> Result<bool, CAStorageError> {
        let tree = self.data.read()?;
        Ok(tree.get(digest).is_some())
    }

    #[instrument(skip(self), fields(subsys = "CaStorage"))]
    fn digests(
        &self,
    ) -> Result<impl Iterator<Item = Result<AnyDigest, CAStorageError>>, CAStorageError> {
        let tree = self.data.read()?;
        let it: Vec<_> = tree.keys().map(|d| Ok(d.clone())).collect();
        Ok(it.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::storage::tests::castorage;

    #[test]
    fn test_set_and_get() {
        let store = MemoryStorage::new();
        castorage::test_set_and_get(store);
    }

    #[test]
    fn test_reset() {
        let store = MemoryStorage::new();
        castorage::test_reset(store);
    }

    #[test]
    fn test_multiple_keys() {
        let store = MemoryStorage::new();
        castorage::test_multiple_keys(store);
    }

    #[test]
    fn test_list_digests() {
        let store = MemoryStorage::new();
        castorage::test_list_digests(store);
    }
}
