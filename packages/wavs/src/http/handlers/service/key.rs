use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{ServiceID, SigningKeyResponse};

#[utoipa::path(
    get,
    path = "/service-key/{service_id}",
    params(
        ("service_id" = String, Path, description = "Unique identifier for the service")
    ),
    responses(
        (status = 200, description = "Service key retrieved successfully", body = SigningKeyResponse),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Retrieves the key associated with a specific service"
)]
#[axum::debug_handler]
pub async fn handle_get_service_key(
    State(state): State<HttpState>,
    axum::extract::Path(service_id): axum::extract::Path<ServiceID>,
) -> impl IntoResponse {
    match inner(&state, service_id).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn inner(state: &HttpState, service_id: ServiceID) -> HttpResult<SigningKeyResponse> {
    Ok(state.dispatcher.get_service_key(service_id)?)
}
