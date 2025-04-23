use axum::{extract::State, http::Response, response::IntoResponse, Json};
use wavs_types::aggregator::RegisterServiceRequest;

use crate::http::{error::*, state::HttpState};

#[utoipa::path(
    post,
    path = "/register-service",
    request_body = RegisterServiceRequest,
    responses(
        (status = 200, description = "Service successfully registered"),
        (status = 400, description = "Invalid service configuration"),
        (status = 500, description = "Internal server error during service registration")
    ),
    description = "Registers a new service with the aggregator. The service definition includes workflows, aggregation settings, and contract details."
)]
#[axum::debug_handler]
pub async fn handle_register_service(
    State(state): State<HttpState>,
    Json(req): Json<RegisterServiceRequest>,
) -> impl IntoResponse {
    match state.register_service(&req.service) {
        Ok(_) => Response::new(().into()),
        Err(e) => AnyError::from(e).into_response(),
    }
}
