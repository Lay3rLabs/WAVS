use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error, Serialize, Deserialize)]
pub enum AppError {
    #[error("IO: {0}")]
    Io(String),

    #[error("JSON: {0}")]
    Json(String),

    #[error("TOML: {0}")]
    Toml(String),

    #[error("Settings: {0}")]
    Settings(String),

    #[error("EventEmitter: {0}")]
    EventEmitter(String),

    #[error("Tauri: {0}")]
    Tauri(String),

    #[error("Wavs Config: {0}")]
    WavsConfig(String),

    #[error("Health check: {0}")]
    HealthCheck(String),
}
