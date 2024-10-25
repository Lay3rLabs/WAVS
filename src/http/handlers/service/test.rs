use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::http::state::HttpState;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAppRequest {
    name: String,
    input: Option<serde_json::Value>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAppResponse {
    output: serde_json::Value,
}

#[axum::debug_handler]
pub async fn handle_test_service(
    State(_state): State<HttpState>,
    Json(_req): Json<TestAppRequest>,
) -> impl IntoResponse {
    Json::<[(); 0]>([]).into_response()
}
