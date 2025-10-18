use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpgradeRequest {}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpgradeResponse {
    /// Whether the database was upgraded
    pub upgraded: bool,
    /// Message describing the upgrade result
    pub message: String,
}

#[utoipa::path(
    post,
    path = "/dev/db/upgrade",
    request_body = UpgradeRequest,
    responses(
        (status = 200, description = "Database upgrade completed successfully", body = UpgradeResponse),
        (status = 500, description = "Internal server error during upgrade")
    ),
    description = "Upgrade the aggregator database to the latest version."
)]
#[axum::debug_handler]
pub async fn handle_upgrade(
    State(state): State<HttpState>,
    Json(request): Json<UpgradeRequest>,
) -> impl IntoResponse {
    match inner_handle_upgrade(state, request).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn inner_handle_upgrade(
    state: HttpState,
    _request: UpgradeRequest,
) -> HttpResult<UpgradeResponse> {
    tracing::info!("Upgrade handler called");

    // Perform the database upgrade using the same logic as the CLI
    let upgraded = state.upgrade_database().map_err(|e| {
        tracing::error!("Database upgrade failed: {}", e);
        e
    })?;

    let message = if upgraded {
        "Upgraded database to the latest version".to_string()
    } else {
        "Database is already up to date".to_string()
    };

    tracing::info!("Upgrade completed: {}", message);

    Ok(UpgradeResponse { upgraded, message })
}
