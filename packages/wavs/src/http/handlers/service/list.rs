use std::ops::Bound;

use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::{
        dispatcher::{Permissions, ServiceStatus},
        trigger::Trigger,
        ServiceID,
    },
    http::{error::HttpResult, state::HttpState, types::ShaDigest},
};
use utils::ServiceID;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListServicesResponse {
    // on the wire, for v0.2, it's "apps"
    // however, internally we are calling these "services"
    // so we'll just treat it as a service here, and keep "apps" field for backwards compat
    #[serde(rename = "apps")]
    pub services: Vec<ServiceResponse>,
    pub digests: Vec<ShaDigest>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceResponse {
    pub id: ServiceID,
    pub status: ServiceStatus,
    pub digest: ShaDigest,
    pub trigger: Trigger,
    pub permissions: Permissions,
    pub testable: Option<bool>,
}

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
            services.push(ServiceResponse {
                digest: component.wasm.clone().into(),
                permissions: component.permissions.clone(),
                status: service.status,
                id: service.id.clone(),
                // just first workflow for now
                trigger: match service.workflows.values().next() {
                    None => return Err(anyhow::anyhow!("No workflows found").into()),
                    Some(w) => w.trigger.clone(),
                },
                testable: Some(service.testable),
            });
        }
    }

    let digests = state.dispatcher.list_component_digests()?;
    let digests = digests.into_iter().map(Into::into).collect();

    Ok(ListServicesResponse { services, digests })
}
