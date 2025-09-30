use axum::{extract::State, response::IntoResponse, Json};
use tracing::instrument;

use crate::{
    health::{update_health_status, HealthStatus},
    http::state::HttpState,
};

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Health status", body = HealthStatus),
    ),
    description = "Get health status of chain endpoints"
)]
#[instrument(level = "debug", skip(state))]
#[axum::debug_handler]
pub async fn handle_health(State(state): State<HttpState>) -> impl IntoResponse {
    let chain_configs = state
        .dispatcher
        .trigger_manager
        .chain_configs
        .read()
        .unwrap()
        .clone();

    let _ = update_health_status(&state.health_status, &chain_configs).await;

    let health_status = state.health_status.read().unwrap().clone();
    Json(health_status).into_response()
}
