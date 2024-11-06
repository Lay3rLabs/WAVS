use std::fs::{DirEntry, File};
use std::io::Read;
use std::path::PathBuf;
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

fn filter_dirs(entry: std::io::Result<DirEntry>) -> Option<std::io::Result<DirEntry>> {
    match entry {
        Ok(dir) => match dir.file_type() {
            Ok(ft) => {
                if ft.is_dir() {
                    Some(Ok(dir))
                } else {
                    None
                }
            }
            Err(e) => Some(Err(e)),
        },
        Err(e) => Some(Err(e)),
    }
}

fn read_digests(
    dir: DirEntry,
) -> Result<impl Iterator<Item = Result<Digest, CAStorageError>>, std::io::Error> {
    let digests = std::fs::read_dir(&dir.path())?.map(|entry| {
        let name = entry?.file_name().into_string().unwrap();
        Ok::<_, CAStorageError>(Digest::from_str(&name)?)
    });
    Ok(digests)
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
        // First, collect all the top-level dirs
        let top_dirs: Result<Vec<DirEntry>, std::io::Error> = std::fs::read_dir(&self.data_dir)?
            .filter_map(filter_dirs)
            .collect();
        // now, read the lower ones, and iterate over the files in them
        let iter = top_dirs?.into_iter().flat_map(read_digests);
        Ok(Box::new(iter))

        // let mut all_dirs = vec![];
        // for dir in top_dirs? {
        //     let digests = std::fs::read_dir(&dir.path())?.map(|entry| {
        //         let name = entry?.file_name().into_string().unwrap();
        //         Ok::<_, CAStorageError>(Digest::from_str(&name)?)
        //     });
        //     all_dirs.extend(digests);
        // }
        // let all_dirs: Box<dyn Iterator<Item=Result<DirEntry, std::io::Error>>> = Box::new(top_dirs?.into_iter().flat_map(|dir| {
        //             Ok(std::fs::read_dir(&dir.path())?.filter_map(filter_dirs))
        //         }));

        // let dirs: Result<Vec<DirEntry>, CAStorageError> =
        //     .flat_map(|entry| Ok::<dyn Iterator<Item=Result<DirEntry, CAStorageError>>, CAStorageError>(std::fs::read_dir(&entry?.path())?.filter_map(filter_dirs)))
        //     .collect();

        // let iter = dirs?.into_iter().flat_map(|dir| {
        //     let digests = std::fs::read_dir(&dir.path())?.map(|entry| {
        //         let name = entry?.file_name().into_string().unwrap();
        //         Ok::<_, CAStorageError>(Digest::from_str(&name)?)
        //     });
        //     Ok(digests)
        // });

        // First attempt, looks better but had type issues
        // let iter = std::fs::read_dir(&self.data_dir)?.flat_map(|entry| {
        //     let dir = entry?;
        //     if dir.file_type()?.is_dir() {
        //         Ok(std::fs::read_dir(dir.path())?.map(|entry| {
        //             let name = entry?.file_name().into_string().unwrap();
        //             Ok::<_, CAStorageError>(Digest::from_str(&name)?)
        //         }))
        //     } else {
        //         Err(CAStorageError::Other("unexpected file in data dir".to_string()))
        //     }
        // });
        // Ok(Box::new(iter))
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
