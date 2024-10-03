use crate::{app::App, storage::StorageError};
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
    app::Trigger,
    operator::{instantiate_and_invoke, TriggerRequest},
    storage::Storage,
};

use super::{Host, Operator};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAppRequest {
    name: String,
    input: Option<Value>,
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

    #[error("App was not registered as testable, reregister app with testable field")]
    AppNotTestable,
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

    let app = storage.get_application(&req.name).await?;
    if let Some(App {
        testable: Some(true),
        digest,
        envs,
        trigger,
        ..
    }) = app
    {
        // check if Wasm is already downloaded
        let component = op.storage.get_wasm(&digest, &engine).await?;
        let mut linker: Linker<Host> = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
        wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();
        // setup app cache directory
        let app_cache_path = op.storage.path_for_app_cache(&req.name);
        if !app_cache_path.is_dir() {
            tokio::fs::create_dir(&app_cache_path).await.unwrap();
        }
        let envs = envs.clone();
        let trigger_request = match trigger {
            Trigger::Cron { .. } => TriggerRequest::Cron,
            Trigger::Queue { .. } => {
                if let Some(i) = &req.input {
                    TriggerRequest::Queue(serde_json::to_vec(&i).unwrap())
                } else {
                    TriggerRequest::Queue(vec![])
                }
            }
            _ => return Err(TestAppError::AppNotTestable),
        };

        let output = instantiate_and_invoke(
            &envs,
            &app_cache_path,
            &engine,
            &linker,
            &component,
            trigger_request,
        )
        .await
        .expect("Failed to instantiate component");
        Ok(Json(TestAppResponse {
            output: serde_json::from_slice(&output).unwrap(),
        }))
    } else {
        Err(TestAppError::AppNotTestable)
    }
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
            TestAppError::AppNotTestable => (
                StatusCode::FORBIDDEN,
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
