use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use wavs_types::ServiceManager;

const REGISTRY_FILENAME: &str = "service_registry.json";

#[derive(Serialize, Deserialize)]
struct RegistryFile {
    next_hd_index: u32,
    services: Vec<RegistryEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    pub service_manager: ServiceManager,
    pub hd_index: u32,
}

#[derive(Clone, Debug)]
pub struct ServiceRegistry {
    entries: Arc<RwLock<Vec<RegistryEntry>>>,
    next_hd_index: Arc<RwLock<u32>>,
    path: PathBuf,
}

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Persist error: {0}")]
    Persist(#[from] tempfile::PersistError),

    #[error("Service manager already registered")]
    AlreadyRegistered,
}

impl ServiceRegistry {
    pub fn load(data_dir: &Path) -> Result<Self, RegistryError> {
        let path = data_dir.join(REGISTRY_FILENAME);

        let (entries, next_hd_index) = if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let file: RegistryFile = serde_json::from_str(&contents)?;
            (file.services, file.next_hd_index)
        } else {
            (Vec::new(), 1)
        };

        Ok(Self {
            entries: Arc::new(RwLock::new(entries)),
            next_hd_index: Arc::new(RwLock::new(next_hd_index)),
            path,
        })
    }

    pub fn entries(&self) -> Vec<RegistryEntry> {
        self.entries.read().unwrap().clone()
    }

    pub fn next_hd_index(&self) -> u32 {
        *self.next_hd_index.read().unwrap()
    }

    pub fn append(&self, sm: ServiceManager) -> Result<u32, RegistryError> {
        let mut entries = self.entries.write().unwrap();
        let mut next = self.next_hd_index.write().unwrap();

        // Check for duplicates
        if entries.iter().any(|e| e.service_manager == sm) {
            return Err(RegistryError::AlreadyRegistered);
        }

        let hd_index = *next;
        entries.push(RegistryEntry {
            service_manager: sm,
            hd_index,
        });
        *next = hd_index + 1;

        self.write_locked(&entries, *next)?;
        Ok(hd_index)
    }

    pub fn remove(&self, sm: &ServiceManager) -> Result<(), RegistryError> {
        let mut entries = self.entries.write().unwrap();
        let next = *self.next_hd_index.read().unwrap();

        if let Some(pos) = entries.iter().position(|e| &e.service_manager == sm) {
            entries.remove(pos);
            self.write_locked(&entries, next)?;
        }

        Ok(())
    }

    fn write_locked(
        &self,
        entries: &[RegistryEntry],
        next_hd_index: u32,
    ) -> Result<(), RegistryError> {
        let file = RegistryFile {
            next_hd_index,
            services: entries.to_vec(),
        };
        let json = serde_json::to_string_pretty(&file)?;

        // Atomic write: write to temp file in same directory, then rename
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut tmp =
            tempfile::NamedTempFile::new_in(self.path.parent().unwrap_or_else(|| Path::new(".")))?;
        tmp.write_all(json.as_bytes())?;
        tmp.flush()?;
        tmp.persist(&self.path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use wavs_types::ChainKey;

    fn test_sm(addr: &str) -> ServiceManager {
        ServiceManager::Evm {
            chain: ChainKey::try_from("evm:local").unwrap(),
            address: addr.parse().unwrap(),
        }
    }

    #[test]
    fn empty_file_gives_empty_registry() {
        let dir = TempDir::new().unwrap();
        let reg = ServiceRegistry::load(dir.path()).unwrap();
        assert!(reg.entries().is_empty());
        assert_eq!(reg.next_hd_index(), 1);
    }

    #[test]
    fn append_and_reload() {
        let dir = TempDir::new().unwrap();
        let sm = test_sm("0x0000000000000000000000000000000000000001");

        {
            let reg = ServiceRegistry::load(dir.path()).unwrap();
            let hd_index = reg.append(sm.clone()).unwrap();
            assert_eq!(hd_index, 1);
            assert_eq!(reg.entries().len(), 1);
            assert_eq!(reg.next_hd_index(), 2);
        }

        // Reload from disk
        let reg = ServiceRegistry::load(dir.path()).unwrap();
        assert_eq!(reg.entries().len(), 1);
        assert_eq!(reg.entries()[0].service_manager, sm);
        assert_eq!(reg.entries()[0].hd_index, 1);
        assert_eq!(reg.next_hd_index(), 2);
    }

    #[test]
    fn duplicate_append_rejected() {
        let dir = TempDir::new().unwrap();
        let sm = test_sm("0x0000000000000000000000000000000000000001");

        let reg = ServiceRegistry::load(dir.path()).unwrap();
        reg.append(sm.clone()).unwrap();
        let err = reg.append(sm).unwrap_err();
        assert!(matches!(err, RegistryError::AlreadyRegistered));
    }

    #[test]
    fn remove_deletes_entry_preserves_counter() {
        let dir = TempDir::new().unwrap();
        let sm1 = test_sm("0x0000000000000000000000000000000000000001");
        let sm2 = test_sm("0x0000000000000000000000000000000000000002");

        let reg = ServiceRegistry::load(dir.path()).unwrap();
        reg.append(sm1.clone()).unwrap(); // hd_index=1
        reg.append(sm2.clone()).unwrap(); // hd_index=2
        reg.remove(&sm1).unwrap();

        let entries = reg.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].service_manager, sm2);
        assert_eq!(entries[0].hd_index, 2);
        // Counter stays at 3, not reset
        assert_eq!(reg.next_hd_index(), 3);

        // Reload and verify
        let reg = ServiceRegistry::load(dir.path()).unwrap();
        assert_eq!(reg.entries().len(), 1);
        assert_eq!(reg.next_hd_index(), 3);

        // New append gets hd_index=3, not 2
        let sm3 = test_sm("0x0000000000000000000000000000000000000003");
        let hd_index = reg.append(sm3).unwrap();
        assert_eq!(hd_index, 3);
    }

    #[test]
    fn corrupt_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(REGISTRY_FILENAME);
        std::fs::write(&path, "not valid json!!!").unwrap();

        let err = ServiceRegistry::load(dir.path()).unwrap_err();
        assert!(matches!(err, RegistryError::Json(_)));
    }
}
