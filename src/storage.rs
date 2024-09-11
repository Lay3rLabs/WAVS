use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use thiserror::Error;
use wasmtime::{component::Component, Engine};
mod fs;
use crate::app::App;
use crate::digest::Digest;
pub use fs::*;

/// Trait for Wasmatic node persistent storage implementations.
///
/// Stores registered applications, Wasm components, and application cache.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Reset and remove storage data.
    async fn reset(&self) -> Result<(), StorageError>;

    fn path_for_app_cache(&self, name: &str) -> PathBuf;

    async fn has_wasm(&self, digest: &Digest) -> Result<bool, StorageError>;
    async fn get_wasm(
        &mut self,
        digest: &Digest,
        engine: &Engine,
    ) -> Result<Component, StorageError>;
    async fn add_wasm(
        &mut self,
        digest: &Digest,
        bytes: &[u8],
        engine: &Engine,
    ) -> Result<(), StorageError>;
    async fn list_wasm(&self) -> Result<Vec<Digest>, StorageError>;
    //async fn remove_wasm(&mut self, digest: &Digest) -> Result<(), StorageError>;

    //async fn has_application(&self, name: &str) -> Result<bool, StorageError>;
    async fn get_application(&self, name: &str) -> Result<Option<App>, StorageError>;
    async fn add_application(&mut self, app: App) -> Result<(), StorageError>;
    async fn remove_applications<'a>(
        &mut self,
        names: impl Iterator<Item = &'a str> + Send,
    ) -> Result<(), StorageError>;
    //async fn update_application(&mut self, app: App) -> Result<(), StorageError>;
    async fn list_applications(&self) -> Result<Vec<App>, StorageError>;
}

/// Represents an error returned by storage implementation.
#[derive(Debug, Error)]
pub enum StorageError {
    /// App name already registered and in use.
    #[error("app name `{0}` is already in use")]
    AppNameConflict(String),

    /// App name not found.
    #[error("app name `{0}` not found")]
    AppNameNotFound(String),

    /// Missing Wasm digest.
    #[error("missing Wasm digest `{0}`")]
    MissingWasmDigest(Digest),

    ///// Wasm in use with application.
    //#[error("Wasm is in use with application `{0}`")]
    //WasmInUse(String),
    /// Digest mismatches.
    #[error("incorrect digest, expected `{expected}` but computed `{computed}`")]
    IncorrectDigest { expected: Digest, computed: Digest },

    ///// Missing download URL.
    //#[error("missing download URL")]
    //MissingDownloadUrl,

    ///// Download error.
    //#[error(transparent)]
    //DownloadError(reqwest::Error),
    /// An error occurred while performing a storage operation.
    #[error("{0:?}")]
    Other(#[from] anyhow::Error),

    /// An error occurred while performing a IO.
    #[error("error: {0:?}")]
    IoError(#[from] std::io::Error),
}
