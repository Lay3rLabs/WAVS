use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

/// This takes a top-level data dir and contains all the logic to
/// walk the directory tree two levels deep and return all the filenames of the bottom-level files
/// as parsed digests.
struct DigestIterator {
    dirs: std::fs::ReadDir,
    top_dir: Option<std::fs::ReadDir>,
    second_dir: Option<std::fs::ReadDir>,
}

impl DigestIterator {
    fn new(data_dir: impl AsRef<Path>) -> Result<Self, CAStorageError> {
        let me = DigestIterator {
            dirs: std::fs::read_dir(data_dir.as_ref())?,
            top_dir: None,
            second_dir: None,
        };
        Ok(me)
    }
}

impl Iterator for DigestIterator {
    type Item = Result<Digest, CAStorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut second_dir) = self.second_dir {
            match second_dir.next() {
                Some(Ok(entry)) => {
                    let name = entry.file_name().into_string().unwrap();
                    println!("name: {:?}", name);
                    Some(Digest::from_str(&name).map_err(CAStorageError::from))
                }
                Some(Err(e)) => Some(Err(CAStorageError::IO(e))),
                None => {
                    println!("finished lower-level dir");
                    self.second_dir = None;
                    self.next()
                }
            }
        } else if let Some(ref mut top_dir) = self.top_dir {
            match top_dir.next() {
                Some(Ok(entry)) => {
                    // TODO: check if it is a dir or file, and skip if not a dir
                    println!("opening second-level dir {:?}", entry.path());
                    self.second_dir = Some(std::fs::read_dir(entry.path()).unwrap());
                    self.next()
                }
                Some(Err(e)) => Some(Err(CAStorageError::IO(e))),
                None => {
                    println!("finished top-level dir");
                    self.top_dir = None;
                    self.next()
                }
            }
        } else {
            match self.dirs.next() {
                Some(Ok(dir)) => {
                    // TODO: check if it is a dir or file, and skip if not a dir
                    println!("opening top-level dir {:?}", dir.path());
                    self.top_dir = Some(std::fs::read_dir(dir.path()).unwrap());
                    self.next()
                }
                Some(Err(e)) => Some(Err(CAStorageError::IO(e))),
                None => None,
            }
        }
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

    /// Returns an iterator over all the digests in the storage.
    /// We store these two levels deep (see digest_to_path), so we need to walk the directory tree.
    fn digests(
        &self,
    ) -> Result<Box<dyn Iterator<Item = Result<Digest, CAStorageError>> + '_>, CAStorageError> {
        let iter = DigestIterator::new(&self.data_dir)?;
        Ok(Box::new(iter))
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

    #[test]
    fn test_list_digests() {
        let (store, dir) = setup();
        castorage::test_list_digests(store);
        // it also gets cleaned up with Drop, in case of test failure
        dir.close().unwrap();
    }
}
