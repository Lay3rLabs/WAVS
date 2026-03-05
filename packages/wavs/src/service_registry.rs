use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use wavs_types::{ServiceManager, ServiceStatus};

const REGISTRY_FILENAME: &str = "service_registry.json";
const REGISTRY_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct RegistryFile {
    version: u32,
    next_hd_index: u32,
    services: Vec<RegistryEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RegistryEntry {
    pub service_manager: ServiceManager,
    pub hd_index: u32,
    /// Persisted service status. Defaults to Active for backward compatibility
    /// with registry files that predate this field.
    #[serde(default = "default_status")]
    pub status: ServiceStatus,
}

fn default_status() -> ServiceStatus {
    ServiceStatus::Active
}

#[derive(Debug)]
struct RegistryState {
    entries: Vec<RegistryEntry>,
    next_hd_index: u32,
}

#[derive(Clone, Debug)]
pub struct ServiceRegistry {
    state: Arc<RwLock<RegistryState>>,
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

    #[error("HD index overflow")]
    HdIndexOverflow,

    #[error("unsupported registry version {found}, expected {REGISTRY_VERSION}")]
    UnsupportedVersion { found: u32 },

    #[error("Service manager not found in registry")]
    NotFound,
}

impl ServiceRegistry {
    pub fn load(data_dir: &Path) -> Result<Self, RegistryError> {
        let path = data_dir.join(REGISTRY_FILENAME);

        let (entries, next_hd_index) = if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let file: RegistryFile = serde_json::from_str(&contents)?;
            if file.version != REGISTRY_VERSION {
                return Err(RegistryError::UnsupportedVersion {
                    found: file.version,
                });
            }
            (file.services, file.next_hd_index)
        } else {
            (Vec::new(), 1)
        };

        Ok(Self {
            state: Arc::new(RwLock::new(RegistryState {
                entries,
                next_hd_index,
            })),
            path,
        })
    }

    pub fn entries(&self) -> Vec<RegistryEntry> {
        self.state.read().unwrap().entries.clone()
    }

    pub fn next_hd_index(&self) -> u32 {
        self.state.read().unwrap().next_hd_index
    }

    pub fn append(&self, sm: ServiceManager) -> Result<u32, RegistryError> {
        let mut state = self.state.write().unwrap();

        // Check for duplicates
        if state.entries.iter().any(|e| e.service_manager == sm) {
            return Err(RegistryError::AlreadyRegistered);
        }

        let hd_index = state.next_hd_index;
        let next = hd_index
            .checked_add(1)
            .ok_or(RegistryError::HdIndexOverflow)?;
        state.entries.push(RegistryEntry {
            service_manager: sm,
            hd_index,
            status: ServiceStatus::Active,
        });
        state.next_hd_index = next;

        self.write_locked(&state)?;
        Ok(hd_index)
    }

    pub fn set_status(
        &self,
        sm: &ServiceManager,
        status: ServiceStatus,
    ) -> Result<(), RegistryError> {
        let mut state = self.state.write().unwrap();
        let entry = state
            .entries
            .iter_mut()
            .find(|e| &e.service_manager == sm)
            .ok_or(RegistryError::NotFound)?;
        entry.status = status;
        self.write_locked(&state)?;
        Ok(())
    }

    pub fn remove(&self, sm: &ServiceManager) -> Result<(), RegistryError> {
        let mut state = self.state.write().unwrap();

        if let Some(pos) = state.entries.iter().position(|e| &e.service_manager == sm) {
            state.entries.remove(pos);
            self.write_locked(&state)?;
        }

        Ok(())
    }

    fn write_locked(&self, state: &RegistryState) -> Result<(), RegistryError> {
        let file = RegistryFile {
            version: REGISTRY_VERSION,
            next_hd_index: state.next_hd_index,
            services: state.entries.to_vec(),
        };
        let json = serde_json::to_string_pretty(&file)?;

        // Atomic write: write to temp file in same directory, then rename
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        tmp.write_all(json.as_bytes())?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.path)?;

        // Sync the parent directory so the rename is durable
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }

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
    fn missing_file_gives_empty_registry() {
        let dir = TempDir::new().unwrap();
        let reg = ServiceRegistry::load(dir.path()).unwrap();
        assert!(reg.entries().is_empty());
        assert_eq!(reg.next_hd_index(), 1);
    }

    #[test]
    fn empty_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(REGISTRY_FILENAME);
        std::fs::write(&path, "").unwrap();

        let err = ServiceRegistry::load(dir.path()).unwrap_err();
        assert!(matches!(err, RegistryError::Json(_)));
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
    fn unsupported_version_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(REGISTRY_FILENAME);
        std::fs::write(&path, r#"{"version":999,"next_hd_index":1,"services":[]}"#).unwrap();

        let err = ServiceRegistry::load(dir.path()).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::UnsupportedVersion { found: 999 }
        ));
    }

    #[test]
    fn corrupt_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(REGISTRY_FILENAME);
        std::fs::write(&path, "not valid json!!!").unwrap();

        let err = ServiceRegistry::load(dir.path()).unwrap_err();
        assert!(matches!(err, RegistryError::Json(_)));
    }

    #[test]
    fn set_status_persists_across_reload() {
        let dir = TempDir::new().unwrap();
        let sm = test_sm("0x0000000000000000000000000000000000000001");

        let reg = ServiceRegistry::load(dir.path()).unwrap();
        reg.append(sm.clone()).unwrap();
        assert_eq!(reg.entries()[0].status, ServiceStatus::Active);

        reg.set_status(&sm, ServiceStatus::Paused).unwrap();
        assert_eq!(reg.entries()[0].status, ServiceStatus::Paused);

        // Reload from disk and verify status persisted
        let reg = ServiceRegistry::load(dir.path()).unwrap();
        assert_eq!(reg.entries()[0].status, ServiceStatus::Paused);

        // Resume and verify
        reg.set_status(&sm, ServiceStatus::Active).unwrap();
        let reg = ServiceRegistry::load(dir.path()).unwrap();
        assert_eq!(reg.entries()[0].status, ServiceStatus::Active);
    }

    #[test]
    fn set_status_on_missing_service_errors() {
        let dir = TempDir::new().unwrap();
        let sm = test_sm("0x0000000000000000000000000000000000000001");

        let reg = ServiceRegistry::load(dir.path()).unwrap();
        let err = reg.set_status(&sm, ServiceStatus::Paused).unwrap_err();
        assert!(matches!(err, RegistryError::NotFound));
    }

    #[test]
    fn legacy_registry_without_status_defaults_to_active() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(REGISTRY_FILENAME);
        // Simulate a registry file from before the status field existed
        std::fs::write(
            &path,
            r#"{"version":1,"next_hd_index":2,"services":[{"service_manager":{"evm":{"chain":"evm:local","address":"0x0000000000000000000000000000000000000001"}},"hd_index":1}]}"#,
        )
        .unwrap();

        let reg = ServiceRegistry::load(dir.path()).unwrap();
        assert_eq!(reg.entries().len(), 1);
        assert_eq!(reg.entries()[0].status, ServiceStatus::Active);
    }
}
