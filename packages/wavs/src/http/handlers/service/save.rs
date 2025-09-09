use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::SaveServiceResponse;

use crate::http::{error::HttpResult, state::HttpState};

#[utoipa::path(
    post,
    path = "/services",
    request_body = wavs_types::Service,
    responses(
        (status = 200, description = "Service saved successfully", body = wavs_types::SaveServiceResponse),
        (status = 400, description = "Invalid service data"),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Updates an existing service with new configuration data"
)]
#[axum::debug_handler]
pub async fn handle_save_service(
    State(state): State<HttpState>,
    Json(req): Json<wavs_types::Service>,
) -> impl IntoResponse {
    match save_service_inner(state, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn save_service_inner(
    state: HttpState,
    service: wavs_types::Service,
) -> HttpResult<SaveServiceResponse> {
    // this does NOT save to the dispatcher, it's just for testing purposes, basically simulating IPFS
    // the url derived from here is typically used to create a ServiceManager instance, e.g. via SetServiceURI
    let service_hash = state.save_service_by_hash(&service)?;
    Ok(SaveServiceResponse { hash: service_hash })
}
