use axum::{extract::State, http::StatusCode, Json};
use tracing::instrument;

use crate::{health::HealthStatus, http::state::HttpState};

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
    let health_status = state.health_status.read().unwrap().clone();
    Ok(Json(health_status))
}
