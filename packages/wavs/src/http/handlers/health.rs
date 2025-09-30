use axum::{extract::State, http::StatusCode, Json};
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
pub async fn handle_health(
    State(state): State<HttpState>,
) -> Result<Json<HealthStatus>, StatusCode> {
    let chain_configs = state
        .dispatcher
        .trigger_manager
        .chain_configs
        .read()
        .unwrap()
        .clone();
    let chains = chain_configs
        .all_chain_keys()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    update_health_status(&state.health_status, &chain_configs, &chains)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let health_status = state.health_status.read().unwrap().clone();
    Ok(Json(health_status))
}
