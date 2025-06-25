use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{Digest, SaveServiceResponse};

use crate::http::{error::HttpResult, state::HttpState};

#[utoipa::path(
    post,
    path = "/save-service",
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
    let service_bytes = serde_json::to_vec(&service)?;
    let service_hash = Digest::new(&service_bytes);
    if state.load_service(&service_hash).is_ok() {
        return Err(anyhow::anyhow!(
            "Service Hash {} has already been set on the http server",
            service_hash
        )
        .into());
    }

    state.save_service(&service_hash, &service)?;

    Ok(SaveServiceResponse { hash: service_hash })
}
