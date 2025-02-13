use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use std::ops::Bound;
use wavs_types::{ListServiceResponse, ListServicesResponse};

#[axum::debug_handler]
pub async fn handle_list_services(State(state): State<HttpState>) -> impl IntoResponse {
    match list_services_inner(&state).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn list_services_inner(state: &HttpState) -> HttpResult<ListServicesResponse> {
    let services_list = state
        .dispatcher
        .list_services(Bound::Unbounded, Bound::Unbounded)?;

    let mut services = Vec::with_capacity(services_list.len());

    // for backwards compatibility, we do some funky things here
    // it will be nicer in 0.3
    for service in services_list {
        for component in service.components.values() {
            services.push(ListServiceResponse {
                digest: component.wasm.clone().into(),
                permissions: component.permissions.clone(),
                status: service.status,
                id: service.id.clone(),
                // just first workflow for now
                trigger: match service.workflows.values().next() {
                    None => return Err(anyhow::anyhow!("No workflows found").into()),
                    Some(w) => w.trigger.clone(),
                },
            });
        }
    }

    let digests = state.dispatcher.list_component_digests()?;
    let digests = digests.into_iter().map(Into::into).collect();

    Ok(ListServicesResponse { services, digests })
}
