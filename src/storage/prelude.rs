use std::fmt::Debug;
use thiserror::Error;

use crate::digest::{Digest, DigestError};

/*
  Documenting a design decisions here:

  Big question if we want this trait sync or async.
  We can implement fs backend async.
  Memory is fine sync or async (not blocking)
  EmbeddedDB backends seem to be all sync:
   - ReDB https://github.com/cberner/redb/issues/30
   - RocksDB https://docs.rs/rocksdb/latest/rocksdb/
  I could wrap ReDB in "tokio::block_in_place" to make it async friendly.
  Or just use the sync version of fs and do the async->sync conversion at a higher level

  Looking more at the wasmtime wasi engine we use and while there are some async traits available,
  It seems that there are hidden sync calls in functions we use ([like `preopened_dir`](https://docs.rs/wasmtime-wasi/26.0.0/src/wasmtime_wasi/ctx.rs.html#327-353))
  Rather than hope and just end up calling blocking calls in our async code, I would make all the engine
  stuff sync and wrap it at a higher-level, where we enter the engine (from http request or triggers).
*/

// TODO: make multi-thread safe - remove &mut by wrapping internally with Arc / RwLock

/// Trait for content-addressable storage. With immutible data on one key.
/// This is what is used for WASM code, stored by hash digest.
pub trait CAStorage: Send + Sync {
    /// Reset and remove storage data.
    fn reset(&self) -> Result<(), CAStorageError>;

    /// Stores the given data and returns the digest to look it up later.
    /// If the data was already stored, this is a no-op but still returns the digest with no error.
    fn set_data(&self, data: &[u8]) -> Result<Digest, CAStorageError>;

    /// Looks up the data for a given digest and returns it. If data not present, returns CAStorageError::NotFound(_)
    fn get_data(&self, digest: &Digest) -> Result<Vec<u8>, CAStorageError>;

    fn digests(
        &self,
    ) -> Result<Box<dyn Iterator<Item = Result<Digest, CAStorageError>> + '_>, CAStorageError>;
}

/// Represents an error returned by storage implementation.
#[derive(Debug, Error)]
pub enum CAStorageError {
    #[error("Digest not found: {0}")]
    NotFound(Digest),

    #[error("{0}")]
    Digest(#[from] DigestError),

    /// An error occurred doing IO in the storage implementation
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    /// An error occurred doing locking in the storage implementation
    #[error("Poisoned Lock error")]
    PoisonedLock,

    #[error("Other: {0}")]
    Other(String),
}

impl<T> From<std::sync::PoisonError<T>> for CAStorageError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        CAStorageError::PoisonedLock
    }
}
