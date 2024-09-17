use super::Operator;
use crate::digest::Digest;
use crate::storage::{Storage, StorageError};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("internal error: `{0}`")]
    InternalServerError(String),
    /// An error occurred while performing a storage operation.
    #[error("{0:?}")]
    Storage(#[from] StorageError),
}
pub async fn upload<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
    bytes: Bytes,
) -> Result<String, UploadError> {
    let digest = Digest::new_sha_256(&bytes);
    let op = operator.clone();
    let mut op = op.try_lock().or(Err(UploadError::InternalServerError(
        "please retry".to_string(),
    )))?;
    let engine = op.engine().clone();
    let storage = op.storage_mut();
    if !storage.has_wasm(&digest).await? {
        storage
            .add_wasm(&digest, &bytes, &engine)
            .await
            .map_err(UploadError::Storage)?;
    }

    Ok(format!("Stored wasm with digest: {}", digest.hex_encoded()))
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        match self {
            UploadError::Storage(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            UploadError::InternalServerError(_) => (
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
