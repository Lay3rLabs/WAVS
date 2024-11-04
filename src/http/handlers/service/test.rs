use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::ID,
    http::{error::HttpResult, state::HttpState},
};

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
    State(state): State<HttpState>,
    Json(req): Json<TestAppRequest>,
) -> impl IntoResponse {
    let resp = test_service_inner(&state, req).await;
    match resp {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn test_service_inner(state: &HttpState, req: TestAppRequest) -> HttpResult<TestAppResponse> {
    let input = req.input.unwrap_or_default();

    let output_bytes = state
        .dispatcher
        .test_service(ID::new(&req.name)?, serde_json::to_vec(&input)?)?;

    let output = serde_json::from_slice(&output_bytes)?;

    let resp = TestAppResponse { output };

    Ok(resp)
}
