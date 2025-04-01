use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;

use crate::http::{error::HttpResult, state::HttpState};
use wavs_types::AddServiceRequest;

#[axum::debug_handler]
pub async fn handle_add_service(
    State(state): State<HttpState>,
    Json(req): Json<AddServiceRequest>,
) -> impl IntoResponse {
    match add_service_inner(state, req).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_service_inner(state: HttpState, req: AddServiceRequest) -> HttpResult<()> {
    let AddServiceRequest { service } = req;

    state.dispatcher.add_service(service).await?;

    Ok(())
}
