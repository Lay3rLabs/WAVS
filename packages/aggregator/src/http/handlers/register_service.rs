use axum::{extract::State, http::Response, response::IntoResponse, Json};
use tracing::instrument;
use wavs_types::aggregator::RegisterServiceRequest;

use crate::http::{error::*, state::HttpState};

#[utoipa::path(
    post,
    path = "/services",
    request_body = RegisterServiceRequest,
    responses(
        (status = 200, description = "Service successfully registered"),
        (status = 400, description = "Invalid service configuration"),
        (status = 500, description = "Internal server error during service registration")
    ),
    description = "Registers a new service with the aggregator."
)]
#[axum::debug_handler]
#[instrument(level = "info", skip(state, req), fields(service.manager = ?req.service_manager))]
pub async fn handle_register_service(
    State(state): State<HttpState>,
    Json(req): Json<RegisterServiceRequest>,
) -> impl IntoResponse {
    match state.register_service(&wavs_types::ServiceId::from(&req.service_manager)) {
        Ok(_) => Response::new(().into()),
        Err(e) => AnyError::from(e).into_response(),
    }
}
