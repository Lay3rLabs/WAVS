use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::ServiceID;

#[axum::debug_handler]
pub async fn handle_get_service(
    State(state): State<HttpState>,
    axum::extract::Path(service_id): axum::extract::Path<ServiceID>,
) -> impl IntoResponse {
    match get_service_inner(&state, service_id).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_service_inner(
    state: &HttpState,
    service_id: ServiceID,
) -> HttpResult<wavs_types::Service> {
    Ok(state.load_service(&service_id)?)
}
