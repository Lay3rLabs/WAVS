use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use std::ops::Bound;
use wavs_types::ListServicesResponse;

#[utoipa::path(
    get,
    path = "/app",
    responses(
        (status = 200, description = "Successfully retrieved service list", body = ListServicesResponse),
        (status = 500, description = "Internal server error")
    ),
    description = "Returns a list of all services registered with WAVS"
)]
#[axum::debug_handler]
pub async fn handle_list_services(State(state): State<HttpState>) -> impl IntoResponse {
    match list_services_inner(&state).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn list_services_inner(state: &HttpState) -> HttpResult<ListServicesResponse> {
    let services = state
        .dispatcher
        .services
        .list(Bound::Unbounded, Bound::Unbounded)?;

    let component_digests = state.dispatcher.list_component_digests()?;

    let service_ids = services
        .iter()
        .map(|service| service.id())
        .collect::<Vec<_>>();

    Ok(ListServicesResponse {
        services,
        service_ids,
        component_digests,
    })
}
