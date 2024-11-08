use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

use super::Operator;
use crate::app;
use crate::storage::{Storage, StorageError};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppRequest {
    #[serde(flatten)]
    pub app: app::App,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppResponse {
    name: String,
    status: app::Status,
}

/// Adds application.
///
/// If the provided Wasm digest is not already in use, the Wasm is
/// fetched, compiled and stored.
pub async fn add<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
    Json(req): Json<RegisterAppRequest>,
) -> Result<Json<RegisterAppResponse>, AddAppError> {
    // reject if app status is defined in the request
    // TODO
    if req.app.status.is_some() {
        return Err(AddAppError::BadRequest(
            "app status should not be specified in request".to_string(),
        ));
    }

    // TODO add validation wasm

    // validate app
    req.app.validate().map_err(AddAppError::AppError)?;

    let op = operator.clone();
    let mut op = op.try_lock().or(Err(AddAppError::InternalServerError(
        "please retry".to_string(),
    )))?;

    let engine = op.engine().clone();
    let storage = op.storage_mut();

    // check if Wasm is already downloaded
    if !storage.has_wasm(&req.app.digest).await? {
        if let Some(url) = req.wasm_url {
            let bytes = match reqwest::get(url).await {
                Ok(res) => match res.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err(AddAppError::DownloadFailed),
                },
                Err(_) => return Err(AddAppError::WasmNotFound),
            };

            storage
                .add_wasm(&req.app.digest, &bytes, &engine)
                .await
                .map_err(AddAppError::Storage)?;
        }
    }

    let name = req.app.name.clone();
    storage
        .add_application(req.app)
        .await
        .map_err(AddAppError::Storage)?;

    op.activate_app(&name).await?;

    Ok(Json(RegisterAppResponse {
        name,
        status: app::Status::Active,
    }))
}

#[derive(Debug, Error)]
pub enum AddAppError {
    #[error("internal error: `{0}`")]
    InternalServerError(String),

    #[error("Wasm URL was not found")]
    WasmNotFound,

    #[error("Wasm URL download failed")]
    DownloadFailed,

    #[error("bad request: `{0}`")]
    BadRequest(String),

    #[error("{0:?}")]
    AppError(app::AppError),

    /// An error occurred while performing a storage operation.
    #[error("{0:?}")]
    Storage(#[from] StorageError),

    /// An error occurred.
    #[error("{0:?}")]
    Other(#[from] anyhow::Error),

    /// An error occurred while performing a IO.
    #[error("error: {0:?}")]
    IoError(#[from] std::io::Error),
}

impl IntoResponse for AddAppError {
    fn into_response(self) -> Response {
        match self {
            AddAppError::Storage(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::AppError(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::WasmNotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::DownloadFailed => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::BadRequest(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    message: String,
}
