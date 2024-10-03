use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::Operator;
use crate::storage::Storage;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInfo {
    pub operators: Vec<String>,
}

pub async fn get<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
) -> Result<Json<GetInfo>, StatusCode> {
    let op = operator.clone();
    let mut op = op.try_lock().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    // let storage = op.storage();

    let mut operators = Vec::<String>::new();
    for hd_index in 0..10 {
        let client = op
            .queue_executor
            .builder
            .hd_index(hd_index)
            .build()
            .await
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        let addr = client.sender().to_string();
        operators.push(addr);
    }

    Ok(Json(GetInfo { operators }))
}
