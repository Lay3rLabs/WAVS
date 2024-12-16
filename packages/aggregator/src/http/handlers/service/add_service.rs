use axum::{extract::State, response::IntoResponse, Json};
use utils::hello_world::{AddAggregatorServiceRequest, AddServiceResponse};

use crate::http::{error::HttpResult, state::HttpState};

#[axum::debug_handler]
pub async fn handle_add_service(
    State(state): State<HttpState>,
    Json(req): Json<AddAggregatorServiceRequest>,
) -> impl IntoResponse {
    match add_service(state, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn add_service(
    state: HttpState,
    req: AddAggregatorServiceRequest,
) -> HttpResult<AddServiceResponse> {
    state.register_service(req.service)?;
    Ok(AddServiceResponse {})
}
