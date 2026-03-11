use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::http::{error::HttpResult, state::HttpState};
use wavs_types::{ManageServiceRequest, ServiceId};

#[utoipa::path(
    post,
    path = "/services/pause",
    request_body = ManageServiceRequest,
    responses(
        (status = 204, description = "Service successfully paused"),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Pauses a registered service, halting trigger execution without removing it"
)]
#[axum::debug_handler]
pub async fn handle_pause_service(
    State(state): State<HttpState>,
    Json(req): Json<ManageServiceRequest>,
) -> impl IntoResponse {
    match pause_service_inner(state, req).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn pause_service_inner(state: HttpState, req: ManageServiceRequest) -> HttpResult<()> {
    let id = ServiceId::from(&req.service_manager);
    state.dispatcher.pause_service(id).await?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/services/resume",
    request_body = ManageServiceRequest,
    responses(
        (status = 204, description = "Service successfully resumed"),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Resumes a paused service, re-enabling trigger execution"
)]
#[axum::debug_handler]
pub async fn handle_resume_service(
    State(state): State<HttpState>,
    Json(req): Json<ManageServiceRequest>,
) -> impl IntoResponse {
    match resume_service_inner(state, req).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn resume_service_inner(state: HttpState, req: ManageServiceRequest) -> HttpResult<()> {
    let id = ServiceId::from(&req.service_manager);
    state.dispatcher.resume_service(id).await?;
    Ok(())
}
