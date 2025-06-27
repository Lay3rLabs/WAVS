use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::SaveServiceResponse;

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
    if let Ok(old_service) = state.load_service(&service.id) {
        if old_service.hash()? == service.hash()? {
            return Err(anyhow::anyhow!(
                "Service {} has already been set on the http server with the same hash",
                service.id
            )
            .into());
        }
    }

    state.save_service(&service)?;

    Ok(SaveServiceResponse { id: service.id })
}
