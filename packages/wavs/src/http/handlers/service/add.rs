use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;

use crate::http::{
    error::HttpResult,
    handlers::services::status::post::{post_service_status, ServiceStatusRequest},
    state::HttpState,
};
use wavs_types::AddServiceRequest;

#[utoipa::path(
    post,
    path = "/app",
    request_body = AddServiceRequest,
    responses(
        (status = 204, description = "Service successfully added"),
        (status = 400, description = "Invalid service configuration"),
        (status = 409, description = "Service already exists"),
        (status = 500, description = "Internal server error")
    ),
    description = "Registers a new service with WAVS"
)]
#[axum::debug_handler]
pub async fn handle_add_service(
    State(state): State<HttpState>,
    Json(req): Json<AddServiceRequest>,
) -> impl IntoResponse {
    match add_service_inner(state, req).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_service_inner(state: HttpState, req: AddServiceRequest) -> HttpResult<()> {
    let AddServiceRequest {
        chain_name,
        address,
        is_enabled,
    } = req;

    let service = state.dispatcher.add_service(chain_name, address).await?;

    // Services are enabled by default, so only update state if we need to disable on init
    if !is_enabled.unwrap_or(true) {
        post_service_status(
            &state,
            service.id,
            ServiceStatusRequest { is_enabled: false },
        )
        .await?;
    }

    state.metrics.increment_registered_services();

    Ok(())
}
