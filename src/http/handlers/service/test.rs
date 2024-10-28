use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::http::{error::HttpResult, state::HttpState};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAppRequest {
    pub name: String,
    pub input: Option<serde_json::Value>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestAppResponse {
    pub output: serde_json::Value,
}

#[axum::debug_handler]
pub async fn handle_test_service(
    State(_state): State<HttpState>,
    Json(req): Json<TestAppRequest>,
) -> impl IntoResponse {
    let resp = test_service_inner(&_state, req).await;
    match resp {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn test_service_inner(
    _state: &HttpState,
    req: TestAppRequest,
) -> HttpResult<TestAppResponse> {
    let input = req.input.unwrap_or_default();
    let x = input
        .get("x")
        .context("missing x")?
        .as_f64()
        .context("x is not a number")?;

    let resp = TestAppResponse {
        output: serde_json::json!({
            "y": (x * x)
        }),
    };

    Ok(resp)
}
