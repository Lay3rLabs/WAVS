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
use crate::storage::{Storage, StorageError};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteApps {
    apps: Vec<String>,
}

pub async fn delete<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
    Json(req): Json<DeleteApps>,
) -> Result<Json<DeleteApps>, DeleteAppError> {
    let op = operator.clone();
    let mut op = op.try_lock().or(Err(DeleteAppError::InternalServerError(
        "please retry".to_string(),
    )))?;
    let DeleteApps { apps } = req;

    // deactivate apps
    for name in apps.iter() {
        op.deactivate_app(name).await?;
    }

    // remove from storage
    let storage = op.storage_mut();
    storage
        .remove_applications(apps.iter().map(|name| name.as_str()))
        .await?;

    Ok(Json(DeleteApps { apps }))
}

#[derive(Debug, Error)]
pub enum DeleteAppError {
    #[error("internal error: `{0}`")]
    InternalServerError(String),

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

impl IntoResponse for DeleteAppError {
    fn into_response(self) -> Response {
        match self {
            DeleteAppError::Storage(_) => (
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
