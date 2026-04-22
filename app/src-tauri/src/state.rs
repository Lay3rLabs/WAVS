use std::{path::PathBuf, sync::Arc, thread::JoinHandle};

use opentelemetry_sdk::metrics::SdkMeterProvider;
use tauri::{AppHandle, Manager};
use utils::{config::ConfigBuilder, storage::fs::FileStorage};
use wavs::{dispatcher::Dispatcher, log_buffer::LogBuffer};
use wavs_gui_shared::{
    error::{AppError, AppResult},
    event::{SettingsEvent, TauriEventEmitterExt},
    settings::Settings,
};
use wavs_types::{ChainConfigs, Credential};

pub struct SettingsState {
    pub path: PathBuf,
    pub inner: std::sync::RwLock<Settings>,
}

impl SettingsState {
    pub async fn load_or_new(app: &AppHandle) -> AppResult<Self> {
        let mut _self = Self::new(app).await?;

        if let Ok(settings) = Self::load_inner(&_self.path).await {
            *_self.inner.write().unwrap() = settings;
        }

        Ok(_self)
    }

    async fn new(app: &AppHandle) -> AppResult<Self> {
        let config_dir = app
            .path()
            .app_config_dir()
            .map_err(|e| AppError::Tauri(e.to_string()))?;

        tokio::fs::create_dir_all(&config_dir)
            .await
            .map_err(|e| AppError::Io(e.to_string()))?;

        let path = config_dir.join("settings.json");

        Ok(Self {
            path,
            inner: std::sync::RwLock::new(Settings::default()),
        })
    }
    async fn load_inner(path: &PathBuf) -> AppResult<Settings> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| AppError::Io(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| AppError::Json(e.to_string()))
    }

    pub async fn update(&self, app: &AppHandle, mut f: impl FnMut(&mut Settings)) -> AppResult<()> {
        let mut settings = { self.inner.write().unwrap().clone() };

        f(&mut settings);

        // Write updated settings back to in-memory state
        *self.inner.write().unwrap() = settings.clone();

        let bytes =
            serde_json::to_vec_pretty(&settings).map_err(|e| AppError::Json(e.to_string()))?;

        app.emit_ext(SettingsEvent { settings })?;

        tokio::fs::write(&self.path, bytes)
            .await
            .map_err(|e| AppError::Io(e.to_string()))?;
        Ok(())
    }

    pub fn get_cloned(&self) -> Settings {
        self.inner.read().unwrap().clone()
    }
}

#[derive(Default)]
pub struct WavsConfigState {
    pub inner: std::sync::RwLock<Option<wavs::config::Config>>,
}

impl WavsConfigState {
    pub async fn load_or_default(path: PathBuf) -> Self {
        match Self::_load_inner(path).await {
            Ok(config) => Self {
                inner: std::sync::RwLock::new(Some(config)),
            },
            Err(_) => Self::default(),
        }
    }

    pub async fn reload(&self, path: PathBuf) -> AppResult<()> {
        let config = Self::_load_inner(path).await?;

        *self.inner.write().unwrap() = Some(config);

        Ok(())
    }

    pub fn chain_configs(&self) -> ChainConfigs {
        match self.inner.read().unwrap().as_ref() {
            Some(config) => config.chains.read().unwrap().clone(),
            None => ChainConfigs::default(),
        }
    }

    pub fn is_set(&self) -> bool {
        self.inner.read().unwrap().is_some()
    }

    pub fn get_cloned(&self) -> Option<wavs::config::Config> {
        self.inner.read().unwrap().clone()
    }

    #[allow(clippy::field_reassign_with_default)]
    async fn _load_inner(path: PathBuf) -> AppResult<wavs::config::Config> {
        let mut args = wavs::args::CliArgs::default();
        args.home = Some(path.clone());

        std::env::remove_var("WAVS_HOME");
        std::env::remove_var("WAVS_DOTENV");
        std::env::remove_var("WAVS_DATA");

        let config: wavs::config::Config = ConfigBuilder::new(args)
            .build()
            .map_err(|e| AppError::WavsConfig(e.to_string()))?;

        Ok(config)
    }
}

#[derive(Default)]
pub struct WavsInstanceState {
    inner: std::sync::RwLock<Option<WavsInstance>>,
}

impl WavsInstanceState {
    pub fn set(&self, instance: WavsInstance) {
        *self.inner.write().unwrap() = Some(instance);
    }

    pub fn dispatcher(&self) -> AppResult<Arc<Dispatcher<FileStorage>>> {
        let guard = self.inner.read().unwrap();
        let instance = guard.as_ref().ok_or(AppError::WavsNotRunning)?;
        Ok(instance.dispatcher.clone())
    }
}

/// In-memory cache for the OS keyring mnemonic.
/// - None: cache cold, keyring not yet accessed
/// - Some(None): keyring checked, no mnemonic exists
/// - Some(Some(cred)): mnemonic cached with Zeroize protection
#[derive(Default)]
pub struct MnemonicCacheState {
    inner: std::sync::RwLock<Option<Option<Credential>>>,
}

impl MnemonicCacheState {
    pub fn get_cached(&self) -> Option<Option<Credential>> {
        self.inner.read().unwrap().clone()
    }

    pub fn set(&self, value: Option<Credential>) {
        *self.inner.write().unwrap() = Some(value);
    }

    pub fn clear(&self) {
        *self.inner.write().unwrap() = Some(None);
    }
}

#[allow(dead_code)]
pub struct WavsInstance {
    pub dispatcher: Arc<Dispatcher<FileStorage>>,
    pub ctx: utils::context::AppContext,
    pub meter_provider: Option<SdkMeterProvider>,
    pub handle: JoinHandle<()>,
}

pub struct LogBufferState {
    pub inner: LogBuffer,
}

#[derive(Default)]
pub struct McpServerState {
    inner: std::sync::Mutex<Option<std::process::Child>>,
}

impl McpServerState {
    pub fn set(&self, child: std::process::Child) {
        *self.inner.lock().unwrap() = Some(child);
    }

    pub fn stop(&self) {
        if let Some(mut child) = self.inner.lock().unwrap().take() {
            let _ = child.kill();
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.inner.lock().unwrap().as_ref().map(|c| c.id())
    }

    pub fn is_running(&self) -> bool {
        let mut guard = self.inner.lock().unwrap();
        if let Some(child) = guard.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited — clear the handle
                    *guard = None;
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }
}
