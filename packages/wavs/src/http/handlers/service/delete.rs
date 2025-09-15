use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::http::{error::HttpResult, state::HttpState};
use wavs_types::{DeleteServicesRequest, ServiceId, ServiceManager};

#[utoipa::path(
    delete,
    path = "/services",
    request_body = DeleteServicesRequest,
    responses(
        (status = 204, description = "Service successfully deleted"),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Removes the registered services from WAVS"
)]
#[axum::debug_handler]
pub async fn handle_delete_service(
    State(state): State<HttpState>,
    Json(req): Json<DeleteServicesRequest>,
) -> impl IntoResponse {
    match delete_service_inner(state, req.service_managers).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn delete_service_inner(
    state: HttpState,
    service_managers: Vec<ServiceManager>,
) -> HttpResult<()> {
    for service_manager in service_managers {
        let id = ServiceId::from(&service_manager);
        state.dispatcher.remove_service(id)?;
        state.metrics.decrement_registered_services();
    }

    Ok(())
}
