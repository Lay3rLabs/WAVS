use super::{Storage, StorageError};
use crate::app::App;
use crate::digest::Digest;
use crate::lock::FileLock;
use anyhow::{Context, Result};
use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use wasmtime::{component::Component, Engine};

const LOCK_FILE_NAME: &str = ".lock";

pub struct FileSystemStorage {
    _lock: FileLock,
    base_dir: PathBuf,
    wasm_in_memory: IndexMap<Digest, Component>,
    stored: StoredData,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredData {
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    apps: IndexMap<String, App>,
    #[serde(default, skip_serializing_if = "IndexSet::is_empty")]
    wasm_on_disk: IndexSet<Digest>,
}

impl FileSystemStorage {
    /// Attempts to lock the package storage.
    ///
    /// The base directory will be created if it does not exist.
    ///
    /// If the lock cannot be acquired, `Ok(None)` is returned.
    pub async fn try_lock(base_dir: impl Into<PathBuf>) -> Result<Option<Self>> {
        let base_dir = base_dir.into();
        match FileLock::try_open_rw(base_dir.join(LOCK_FILE_NAME))? {
            Some(lock) => {
                // check if need to create "wasm/" directory
                let wasm_dir = base_dir.join("wasm");
                if !wasm_dir.is_dir() {
                    std::fs::create_dir(wasm_dir)?;
                }

                // check if need to create "app/" directory
                let app_dir = base_dir.join("app");
                if !app_dir.is_dir() {
                    std::fs::create_dir(app_dir)?;
                }

                let mut storage = Self {
                    _lock: lock,
                    base_dir,
                    wasm_in_memory: IndexMap::new(),
                    stored: Default::default(),
                };
                storage.load().await?;
                Ok(Some(storage))
            }
            None => Ok(None),
        }
    }

    fn path_for_precompiled_wasm(&self, digest: &Digest) -> PathBuf {
        self.base_dir
            .join(format!("wasm/{digest}.bin", digest = digest.hex_encoded()))
    }
    fn path_for_wasm(&self, digest: &Digest) -> PathBuf {
        self.base_dir
            .join(format!("wasm/{digest}.wasm", digest = digest.hex_encoded()))
    }
    fn path_for_global_app_data(&self) -> PathBuf {
        self.base_dir.join("apps.json")
    }

    async fn save(&self) -> Result<()> {
        let serialized = serde_json::to_vec(&self.stored)?;
        tokio::fs::write(self.path_for_global_app_data(), serialized).await?;
        Ok(())
    }
    async fn load(&mut self) -> Result<()> {
        let path = self.path_for_global_app_data();
        if path.is_file() {
            let serialized = tokio::fs::read(path).await?;
            self.stored = serde_json::from_slice(&serialized)?;
        }
        Ok(())
    }
}

#[async_trait]
impl Storage for FileSystemStorage {
    async fn reset(&self) -> Result<(), StorageError> {
        tokio::fs::remove_dir_all(&self.base_dir)
            .await
            .with_context(|| {
                format!(
                    "failed to remove storage directory `{path}`",
                    path = self.base_dir.display()
                )
            })?;
        Ok(())
    }
    fn path_for_app_cache(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!(
            "app/{name_hash}",
            name_hash = Digest::new_sha_256(name.as_bytes())
        ))
    }

    async fn has_wasm(&self, digest: &Digest) -> Result<bool, StorageError> {
        Ok(self.stored.wasm_on_disk.contains(digest))
    }
    async fn get_wasm(
        &mut self,
        digest: &Digest,
        engine: &Engine,
    ) -> Result<Component, StorageError> {
        match self.wasm_in_memory.get(digest) {
            Some(cm) => Ok(cm.clone()),
            None if self.stored.wasm_on_disk.contains(digest) => {
                let path = self.path_for_precompiled_wasm(digest);
                let cm = unsafe { Component::deserialize_file(engine, path)? };
                self.wasm_in_memory.insert(digest.clone(), cm.clone());
                Ok(cm)
            }
            None => return Err(StorageError::MissingWasmDigest(digest.clone())),
        }
    }
    async fn add_wasm(
        &mut self,
        digest: &Digest,
        bytes: &[u8],
        engine: &Engine,
    ) -> Result<(), StorageError> {
        // validate bytes match expected digest
        let computed = Digest::new_sha_256(bytes);
        if digest != &computed {
            return Err(StorageError::IncorrectDigest {
                expected: digest.clone(),
                computed,
            });
        }

        {
            // compile component
            let cm = Component::new(engine, bytes)?;

            // write precompiled wasm
            tokio::fs::write(self.path_for_precompiled_wasm(digest), cm.serialize()?).await?;
        }

        // write wasm file
        tokio::fs::write(self.path_for_wasm(digest), bytes).await?;

        // add the digest to the list of wasm on disk
        self.stored.wasm_on_disk.insert(digest.clone());

        // save stored data to disk
        self.save().await?;

        Ok(())
    }
    async fn list_wasm(&self) -> Result<Vec<Digest>, StorageError> {
        Ok(self.stored.wasm_on_disk.iter().cloned().collect())
    }
    //async fn remove_wasm(&mut self, digest: &Digest) -> Result<(), StorageError> {
    //    // check if in use on an active application
    //    if let Some(app) = self.stored.apps.values().find(|&app| &app.digest == digest) {
    //        return Err(StorageError::WasmInUse(app.name.clone()));
    //    }

    //    tokio::fs::remove_file(self.path_for_precompiled_wasm(digest)).await?;
    //    tokio::fs::remove_file(self.path_for_wasm(digest)).await?;
    //    self.stored.wasm_on_disk.swap_remove(digest);
    //    self.wasm_in_memory.swap_remove(digest);
    //    self.save().await?;
    //    Ok(())
    //}

    async fn get_application(&self, name: &str) -> Result<Option<App>, StorageError> {
        Ok(self.stored.apps.get(name).cloned())
    }

    async fn add_application(&mut self, app: App) -> Result<(), StorageError> {
        // check if the app name already exists
        if self.stored.apps.contains_key(&app.name) {
            return Err(StorageError::AppNameConflict(app.name.clone()));
        }

        // check if has Wasm digest
        if !self.stored.wasm_on_disk.contains(&app.digest) {
            return Err(StorageError::MissingWasmDigest(app.digest.clone()));
        }

        self.stored.apps.insert(app.name.clone(), app);
        self.save().await?;

        Ok(())
    }

    async fn remove_applications<'a>(
        &mut self,
        names: impl Iterator<Item = &'a str> + Send,
    ) -> Result<(), StorageError> {
        // TODO unregister listeners for events and scheduled CRON jobs

        // remove app and keep track of Wasm digests that may be removed,
        // if not in use by other apps
        let mut digests_used = IndexSet::new();
        for name in names {
            if let Some(app) = self.stored.apps.swap_remove(name) {
                digests_used.insert(app.digest);
            } else {
                return Err(StorageError::AppNameNotFound(name.to_string()));
            }
        }

        for digest in digests_used.iter() {
            if !self.stored.apps.values().any(|app| &app.digest == digest) {
                // no longer in use, safe to remove
                tokio::fs::remove_file(self.path_for_precompiled_wasm(digest)).await?;
                tokio::fs::remove_file(self.path_for_wasm(digest)).await?;
                self.stored.wasm_on_disk.swap_remove(digest);
                self.wasm_in_memory.swap_remove(digest);
            }
        }

        self.save().await?;
        Ok(())
    }

    async fn list_applications(&self) -> Result<Vec<App>, StorageError> {
        Ok(self.stored.apps.values().cloned().collect())
    }
}
