use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::Operator;
use crate::storage::Storage;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteApps {
    apps: Vec<String>,
}

pub async fn delete<S: Storage + 'static>(
    State(_operator): State<Arc<Mutex<Operator<S>>>>,
    Json(_req): Json<DeleteApps>,
) -> Json<DeleteApps> {
    Json(DeleteApps { apps: vec![] })
}
