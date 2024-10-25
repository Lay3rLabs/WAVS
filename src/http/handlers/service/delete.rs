use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::http::state::HttpState;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteApps {
    apps: Vec<String>,
}

#[axum::debug_handler]
pub async fn handle_delete_service(
    State(_state): State<HttpState>,
    Json(_req): Json<DeleteApps>,
) -> impl IntoResponse {
    Json::<[(); 0]>([]).into_response()
}
