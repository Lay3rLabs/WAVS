use axum::{extract::State, http::Response, response::IntoResponse, Json};
use utils::aggregator::AddAggregatorServiceRequest;

use crate::http::{error::*, state::HttpState};

#[axum::debug_handler]
pub async fn handle_add_service(
    State(state): State<HttpState>,
    Json(req): Json<AddAggregatorServiceRequest>,
) -> impl IntoResponse {
    match add_service(state, req).await {
        Ok(_) => Response::new(().into()),
        Err(e) => AnyError::from(e).into_response(),
    }
}

pub async fn add_service(state: HttpState, req: AddAggregatorServiceRequest) -> anyhow::Result<()> {
    match req {
        AddAggregatorServiceRequest::EthTrigger {
            service_manager_address,
            service_id,
        } => state.register_service(service_manager_address, service_id),
    }
}
