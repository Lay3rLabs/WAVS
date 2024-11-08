use axum::response::{IntoResponse, Response};
use axum::{extract::State, http::StatusCode, Json};

use cw_orch::daemon::DaemonError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Mutex, TryLockError};

use super::Operator;
use crate::storage::Storage;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInfo {
    pub operators: Vec<String>,
}

pub async fn get<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
) -> Result<Json<GetInfo>, GetInfoError> {
    let op = operator.clone();
    let mut op = op.try_lock()?;
    // let storage = op.storage();

    let mut operators = Vec::<String>::new();
    for hd_index in 0..5 {
        // FIXME: this is EXTREMELY inefficient as it makes full grpc client for eaxch just to get
        // the address. I don't see better access to DaemonBuilder internals, so we should
        // store other info in the Operator struct that allows quick computation of senders.
        let client = op.queue_executor.builder.hd_index(hd_index).build().await?;
        let addr = client.sender().to_string();
        operators.push(addr);
    }

    Ok(Json(GetInfo { operators }))
}

#[derive(Debug, Error)]
pub enum GetInfoError {
    #[error("daemon: {0}")]
    Daemon(#[from] DaemonError),

    #[error("lock: {0}")]
    Lock(#[from] TryLockError),
}

impl IntoResponse for GetInfoError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorMessage {
                message: self.to_string(),
            }),
        )
            .into_response()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    message: String,
}
