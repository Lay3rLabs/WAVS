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
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)?;
        }
        // TODO: else check this is a valid dir we can write to
        Ok(FileStorage { data_dir })
    }
}

impl CAStorage for FileStorage {
    fn reset(&mut self) -> Result<(), CAStorageError> {
        // wipe out and re-create the entire directory
        std::fs::remove_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
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

#[cfg(test)]
mod tests {
    use tempfile::{tempdir, TempDir};

    use super::*;
    use crate::storage::tests::castorage;

    fn setup() -> (FileStorage, TempDir) {
        let dir = tempdir().unwrap();
        let store = FileStorage::new(dir.path()).unwrap();
        (store, dir)
    }

    #[test]
    fn test_set_and_get() {
        let (store, dir) = setup();
        castorage::test_set_and_get(store);
        // it also gets cleaned up with Drop, in case of test failure
        dir.close().unwrap();
    }

    #[test]
    fn test_reset() {
        let (store, dir) = setup();
        castorage::test_reset(store);
        // it also gets cleaned up with Drop, in case of test failure
        dir.close().unwrap();
    }
}
