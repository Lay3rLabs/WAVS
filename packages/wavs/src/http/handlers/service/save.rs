use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;

use crate::http::{error::HttpResult, state::HttpState};

#[axum::debug_handler]
pub async fn handle_save_service(
    State(state): State<HttpState>,
    Json(req): Json<wavs_types::Service>,
) -> impl IntoResponse {
    match save_service_inner(state, req).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn save_service_inner(state: HttpState, service: wavs_types::Service) -> HttpResult<()> {
    if state.load_service(&service.id).is_ok() {
        return Err(anyhow::anyhow!(
            "Service ID {} has already been set on the http server",
            service.id
        )
        .into());
    }

    Ok(state.save_service(&service)?)
}
