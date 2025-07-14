use std::str::FromStr;

use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{ServiceDigest, ServiceID};

#[utoipa::path(
    get,
    path = "/service/{service_id}",
    params(
        ("service_id" = String, Path, description = "Unique identifier for the service")
    ),
    responses(
        (status = 200, description = "Service found", body = wavs_types::Service),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Retrieves detailed information about a specific service by its ID"
)]
#[axum::debug_handler]
pub async fn handle_get_service(
    State(state): State<HttpState>,
    axum::extract::Path(service_id): axum::extract::Path<ServiceID>,
) -> impl IntoResponse {
    match get_service_inner(&state, service_id).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_service_inner(
    state: &HttpState,
    service_id: ServiceID,
) -> HttpResult<wavs_types::Service> {
    Ok(state.load_service(&service_id)?)
}

#[utoipa::path(
    get,
    path = "/service-by-hash/{service_hash}",
    params(
        ("service_hash" = String, Path, description = "Unique hash of the service")
    ),
    responses(
        (status = 200, description = "Service found", body = wavs_types::Service),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Retrieves detailed information about a specific service by its Hash (for testing only)"
)]
#[axum::debug_handler]
pub async fn handle_get_service_by_hash(
    State(state): State<HttpState>,
    axum::extract::Path(service_hash): axum::extract::Path<String>,
) -> impl IntoResponse {
    match get_service_inner_hash(&state, service_hash).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_service_inner_hash(
    state: &HttpState,
    service_hash: String,
) -> HttpResult<wavs_types::Service> {
    let service_hash = ServiceDigest::from_str(&service_hash)?;

    Ok(state.load_service_by_hash(&service_hash)?)
}
