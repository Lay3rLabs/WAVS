use std::str::FromStr;

use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{Digest, ServiceID};

#[utoipa::path(
    get,
    path = "/service/{service_hash}",
    params(
        ("service_hash" = String, Path, description = "Unique identifier for the service")
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
    axum::extract::Path(service_hash): axum::extract::Path<String>,
) -> impl IntoResponse {
    match get_service_inner(&state, service_hash).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_service_inner(
    state: &HttpState,
    service_hash: String,
) -> HttpResult<wavs_types::Service> {
    tracing::warn!("Fetching service with hash: {}", service_hash);
    let service_hash = Digest::from_str(&service_hash)
        .map_err(|_| anyhow::anyhow!("Invalid service hash format: {}", service_hash))?;
    Ok(state.load_service(&service_hash)?)
}
