use axum::{extract::State, response::IntoResponse, Json};

use crate::http::state::HttpState;

/// GET /p2p/status - Get the current P2P network status
pub async fn handle_p2p_status(State(state): State<HttpState>) -> impl IntoResponse {
    let status = state.dispatcher.aggregator.get_p2p_status().await;
    Json(status)
}
