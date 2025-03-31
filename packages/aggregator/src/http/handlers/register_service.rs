use axum::{extract::State, http::Response, response::IntoResponse, Json};
use wavs_types::aggregator::RegisterServiceRequest;

use crate::http::{error::*, state::HttpState};

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
