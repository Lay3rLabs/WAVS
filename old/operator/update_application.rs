use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
//use wasmtime::{
//    component::{Component, Linker},
//    Config, Engine, Memory, MemoryType, Module, Precompiled, Store, StoreLimits,
//    StoreLimitsBuilder,
//};

use super::Operator;
use crate::app;
use crate::storage::Storage;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppRequest {
    pub name: String,
    pub wasm_digest: String,
    pub wasm_url: Option<String>,
    pub trigger: app::Trigger,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppResponse {
    name: String,
    status: app::Status,
}

pub async fn update<S: Storage + 'static>(
    State(_operator): State<Arc<Mutex<Operator<S>>>>,
    Json(req): Json<UpdateAppRequest>,
) -> Json<UpdateAppResponse> {
    Json(UpdateAppResponse {
        name: req.name,
        status: app::Status::Failed,
    })
}
