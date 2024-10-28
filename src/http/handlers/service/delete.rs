use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::http::state::HttpState;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteApps {
    pub apps: Vec<String>,
}

#[axum::debug_handler]
pub async fn handle_delete_service(
    State(_state): State<HttpState>,
    Json(_req): Json<DeleteApps>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
