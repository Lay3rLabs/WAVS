use axum::{extract::State, response::IntoResponse, Json};

use crate::http::state::HttpState;

#[axum::debug_handler]
pub async fn handle_config(State(state): State<HttpState>) -> impl IntoResponse {
    Json(state.dispatcher.config()).into_response()
}
