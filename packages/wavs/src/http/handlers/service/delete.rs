use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::http::{error::HttpResult, state::HttpState};
use utils::types::DeleteServicesRequest;
use utils::ServiceID;

#[axum::debug_handler]
pub async fn handle_delete_service(
    State(state): State<HttpState>,
    Json(req): Json<DeleteServicesRequest>,
) -> impl IntoResponse {
    match delete_service_inner(state, req.service_ids).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn delete_service_inner(state: HttpState, service_ids: Vec<ServiceID>) -> HttpResult<()> {
    for id in service_ids {
        state.dispatcher.remove_service(id)?;
    }

    Ok(())
}
