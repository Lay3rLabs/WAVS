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

    /// Find the path to look up the item with the given digest.
    /// We could just store the files directly in the data dir, but that will hit issues when 1000s of files are in there.
    /// We store under `data_dir/<digest[0:2]>/<digest[2:4]>/<digest>`.
    /// This keeps the top two levels to 256 max, and it will be around 65 million files til the last dir has 1000 file descriptors.
    /// Keeping full hash as filename, as it is easier to debug later.
    fn digest_to_path(&self, digest: &Digest) -> Result<PathBuf, CAStorageError> {
        let digest = digest.to_string();
        let dir = self.data_dir.join(&digest[..2]).join(&digest[2..4]);
        self.ensure_dir(&dir)?;
        Ok(dir.join(digest))
    }

    fn ensure_dir(&self, path: &PathBuf) -> Result<(), CAStorageError> {
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

impl CAStorage for FileStorage {
    fn reset(&self) -> Result<(), CAStorageError> {
        // wipe out and re-create the entire directory
        std::fs::remove_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        Ok(())
    }

    /// look for file by key and only write if not present
    fn set_data(&self, data: &[u8]) -> Result<Digest, CAStorageError> {
        let digest = Digest::new(data);
        let path = self.digest_to_path(&digest)?;
        if !path.exists() {
            // Question: do we need file locks?
            std::fs::write(&path, data)?;
        }
        Ok(digest)
    }

    fn get_data(&self, digest: &Digest) -> Result<Vec<u8>, CAStorageError> {
        let path = self.digest_to_path(digest)?;
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

    #[test]
    fn test_multiple_keys() {
        let (store, dir) = setup();
        castorage::test_multiple_keys(store);
        // it also gets cleaned up with Drop, in case of test failure
        dir.close().unwrap();
    }
}
