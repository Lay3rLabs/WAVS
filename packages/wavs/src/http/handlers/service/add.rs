use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;

use crate::http::{
    error::HttpResult, handlers::service::get::get_service_inner_hash, state::HttpState,
};
use wavs_types::{AddServiceRequest, ServiceManager};

#[utoipa::path(
    post,
    path = "/services",
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
    match add_service_inner(state, req.service_manager).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_service_inner(state: HttpState, service_manager: ServiceManager) -> HttpResult<()> {
    state.dispatcher.add_service(service_manager).await?;

    state.metrics.increment_registered_services();

    Ok(())
}

#[utoipa::path(
    post,
    path = "/dev/services/{service_hash}",
    params(
        ("service_hash" = String, Path, description = "Unique hash of the service")
    ),
    responses(
        (status = 204, description = "Service successfully added"),
        (status = 400, description = "Invalid service configuration"),
        (status = 409, description = "Service already exists"),
        (status = 500, description = "Internal server error")
    ),
    description = "Registers a new service with WAVS"
)]
#[axum::debug_handler]
pub async fn handle_add_service_direct(
    State(state): State<HttpState>,
    axum::extract::Path(service_hash): axum::extract::Path<String>,
) -> impl IntoResponse {
    match add_service_direct_inner(state, service_hash).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_service_direct_inner(state: HttpState, service_hash: String) -> HttpResult<()> {
    let service = get_service_inner_hash(&state, service_hash).await?;
    state.dispatcher.add_service_direct(service).await?;

    state.metrics.increment_registered_services();

    Ok(())
}
