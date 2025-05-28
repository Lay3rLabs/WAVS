use axum::{extract::State, http::Response, response::IntoResponse, Json};
use tracing::instrument;
use utils::service::fetch_service;
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
    description = "Registers a new service with the aggregator. The service definition includes workflows, aggregation settings, and contract details."
)]
#[axum::debug_handler]
#[instrument(level = "info", skip(state, req), fields(service_uri = %req.uri))]
pub async fn handle_register_service(
    State(state): State<HttpState>,
    Json(req): Json<RegisterServiceRequest>,
) -> impl IntoResponse {
    match inner(state, req).await {
        Ok(_) => Response::new(().into()),
        Err(e) => AnyError::from(e).into_response(),
    }
}

#[instrument(level = "debug", skip(state), fields(service_uri = %req.uri))]
async fn inner(state: HttpState, req: RegisterServiceRequest) -> Result<(), AggregatorError> {
    tracing::info!("Fetching service from URI: {}", req.uri);
    let service = fetch_service(&req.uri, &state.config.ipfs_gateway)
        .await
        .map_err(AggregatorError::FetchService)?;

    tracing::info!("Registering service: {}", service.id);
    state.register_service(&service)
}
