use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{GetServiceKeyRequest, ServiceId, ServiceManager, SigningKeyResponse};

#[utoipa::path(
    post,
    path = "/service-key",
    request_body = GetServiceKeyRequest,
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
    Json(req): Json<GetServiceKeyRequest>,
) -> impl IntoResponse {
    match inner(&state, req.service_manager).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn inner(
    state: &HttpState,
    service_manager: ServiceManager,
) -> HttpResult<SigningKeyResponse> {
    Ok(state
        .dispatcher
        .get_service_key(ServiceId::from(&service_manager))?)
}
