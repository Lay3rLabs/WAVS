use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;
use utoipa::ToSchema;

use crate::http::{error::HttpResult, state::HttpState};
use wavs_types::ServiceID;

#[derive(serde::Deserialize, ToSchema)]
pub struct ServiceStatusRequest {
    pub is_enabled: bool,
}

#[utoipa::path(
    post,
    path = "/services/{service_id}/status",
    params(
        ("service_id" = String, Path, description = "Unique identifier for the service")
    ),
    request_body = ServiceStatusRequest,
    responses(
        (status = 204, description = "Service status successfully updated"),
        (status = 400, description = "Invalid service configuration"),
        (status = 500, description = "Internal server error")
    ),
    description = "Updates the node's service status"
)]
#[axum::debug_handler]
pub async fn handle_post_service_status(
    State(state): State<HttpState>,
    axum::extract::Path(service_id): axum::extract::Path<ServiceID>,
    Json(req): Json<ServiceStatusRequest>,
) -> impl IntoResponse {
    match post_service_status(&state, service_id, req).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

pub(crate) async fn post_service_status(
    state: &HttpState,
    service_id: ServiceID,
    req: ServiceStatusRequest,
) -> HttpResult<()> {
    state
        .dispatcher
        .services
        .update_node_service_status(service_id, req.is_enabled);

    Ok(())
}
