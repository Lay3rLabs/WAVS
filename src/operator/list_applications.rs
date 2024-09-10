use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::Operator;
use crate::app;
use crate::digest::Digest;
use crate::storage::Storage;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListApps {
    apps: Vec<app::App>,
    digests: Vec<Digest>,
}

pub async fn list<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
) -> Result<Json<ListApps>, StatusCode> {
    let op = operator.clone();
    let op = op.try_lock().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let storage = op.storage();
    let apps = storage
        .list_applications()
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let digests = storage
        .list_wasm()
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok(Json(ListApps { apps, digests }))
}
