use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{GetSignerRequest, ServiceId, ServiceManager, SignerResponse};

#[utoipa::path(
    post,
    path = "/services/signer",
    request_body = GetSignerRequest,
    responses(
        (status = 200, description = "Service signer retrieved successfully", body = SignerResponse),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Retrieves the key associated with a specific service"
)]
#[axum::debug_handler]
pub async fn handle_get_service_signer(
    State(state): State<HttpState>,
    Json(req): Json<GetSignerRequest>,
) -> impl IntoResponse {
    match inner(&state, req.service_manager).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn inner(state: &HttpState, service_manager: ServiceManager) -> HttpResult<SignerResponse> {
    Ok(state
        .dispatcher
        .get_service_signer(ServiceId::from(&service_manager))?)
}
