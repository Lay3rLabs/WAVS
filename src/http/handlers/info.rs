use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::http::state::HttpState;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub operators: Vec<String>,
}

#[axum::debug_handler]
pub async fn handle_info(State(_state): State<HttpState>) -> impl IntoResponse {
    Json::<[(); 0]>([]).into_response()
}
