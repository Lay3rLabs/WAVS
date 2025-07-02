use axum::{extract::State, http::Response, response::IntoResponse, Json};
use tracing::instrument;
use wavs_types::aggregator::RegisterServiceRequest;

use crate::{
    error::AggregatorError,
    http::{error::*, state::HttpState},
};

#[utoipa::path(
    post,
    path = "/register-service",
    request_body = RegisterServiceRequest,
    responses(
        (status = 200, description = "Service successfully registered"),
        (status = 400, description = "Invalid service configuration"),
        (status = 500, description = "Internal server error during service registration")
    ),
    description = "Registers a new service with the aggregator."
)]
#[axum::debug_handler]
#[instrument(level = "info", skip(state, req), fields(service_id = %req.service_id))]
pub async fn handle_register_service(
    State(state): State<HttpState>,
    Json(req): Json<RegisterServiceRequest>,
) -> impl IntoResponse {
    match inner(state, req).await {
        Ok(_) => Response::new(().into()),
        Err(e) => AnyError::from(e).into_response(),
    }
}

#[instrument(level = "debug", skip(state), fields(service_id = %req.service_id))]
async fn inner(state: HttpState, req: RegisterServiceRequest) -> Result<(), AggregatorError> {
    state.register_service(&req.service_id)
}
