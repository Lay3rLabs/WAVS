#![allow(dead_code)]
use anyhow::Result;
use std::sync::{Arc, LazyLock, Mutex};

use crate::route::Route;

#[derive(Debug, Clone)]
pub struct Config {
    local_media_server: bool,
    pub debug: ConfigDebug,
    pub root_path: &'static str,
    pub tauri_invoke_mock: bool,
    pub tauri_event_mock: bool,
}

impl Config {
    pub async fn app_image_url(&self, path: &str) -> Result<String> {
        if self.local_media_server {
            Ok(format!("{}/media/{}", self.root_path, path))
        } else {
            crate::tauri::resource_img_url(&format!("media/{}", path)).await
        }
    }
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let browser_dev = match option_env!("TAURI_BROWSER_DEV") {
        Some(val) if val.to_lowercase() == "true" => true,
        _ => false,
    };
    Config {
        local_media_server: browser_dev,
        tauri_invoke_mock: browser_dev,
        tauri_event_mock: browser_dev,
        debug: if cfg!(debug_assertions) {
            //ConfigDebug::release_mode()
            ConfigDebug::dev_mode()
        } else {
            ConfigDebug::release_mode()
        },
        root_path: if cfg!(debug_assertions) {
            ""
        } else {
            ""
            // if on some public place like github pages: "/my-project"
        },
    }
});

#[derive(Debug, Clone)]
pub struct ConfigDebug {
    pub start_route: Arc<Mutex<Option<Route>>>,
}

impl ConfigDebug {
    fn dev_mode() -> Self {
        Self {
            //start_route: Arc::new(Mutex::new(Some(Route::Logs))),
            start_route: Arc::new(Mutex::new(None)),
        }
    }

    fn release_mode() -> Self {
        Self {
            start_route: Arc::new(Mutex::new(None)),
        }
    }
}
