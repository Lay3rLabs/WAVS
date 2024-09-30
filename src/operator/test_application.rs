use crate::{digest::Digest, storage::StorageError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use wasmtime::component::Linker;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    operator::{instantiate_and_invoke, TriggerRequest},
    storage::Storage,
};

use super::{Host, Operator};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAppRequest {
    name: String,
    digest: Digest,
    envs: Vec<(String, String)>,
    input: Option<Value>,
    wasm_url: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAppResponse {
    output: Value,
}
#[derive(Debug, Error)]
pub enum TestAppError {
    #[error("internal error: `{0}`")]
    InternalServerError(String),

    /// An error occurred while performing a storage operation.
    #[error("{0:?}")]
    Storage(#[from] StorageError),

    #[error("Wasm URL was not found")]
    WasmNotFound,

    #[error("Wasm URL download failed")]
    DownloadFailed,
}
pub async fn test<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
    Json(req): Json<TestAppRequest>,
) -> Result<Json<TestAppResponse>, TestAppError> {
    let op = operator.clone();
    let mut op = op.try_lock().or(Err(TestAppError::InternalServerError(
        "please retry".to_string(),
    )))?;

    let engine = op.engine().clone();
    let storage = op.storage_mut();

    // check if Wasm is already downloaded
    if !storage.has_wasm(&req.digest).await? {
        if let Some(url) = req.wasm_url {
            let bytes = match reqwest::get(url).await {
                Ok(res) => match res.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err(TestAppError::DownloadFailed),
                },
                Err(_) => return Err(TestAppError::WasmNotFound),
            };

            storage
                .add_wasm(&req.digest, &bytes, &engine)
                .await
                .map_err(TestAppError::Storage)?;
        }
    }
    let component = op.storage.get_wasm(&req.digest, &engine).await?;
    let mut linker: Linker<Host> = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();
    // setup app cache directory
    let app_cache_path = op.storage.path_for_app_cache(&req.name);
    if !app_cache_path.is_dir() {
        tokio::fs::create_dir(&app_cache_path).await.unwrap();
    }
    let mut envs = op.envs.clone();
    envs.extend_from_slice(&req.envs);
    let trigger = if let Some(i) = &req.input {
        TriggerRequest::Queue(serde_json::to_vec(&i).unwrap())
    } else {
        TriggerRequest::Cron
    };

    let output = instantiate_and_invoke(
        &envs,
        &app_cache_path,
        &engine,
        &linker,
        &component,
        trigger,
    )
    .await
    .expect("Failed to instantiate component");
    Ok(Json(TestAppResponse {
        output: serde_json::from_slice(&output).unwrap(),
    }))
}

impl IntoResponse for TestAppError {
    fn into_response(self) -> Response {
        match self {
            TestAppError::InternalServerError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            TestAppError::Storage(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            TestAppError::WasmNotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            TestAppError::DownloadFailed => (
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
