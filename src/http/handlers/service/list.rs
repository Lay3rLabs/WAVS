use std::ops::Bound;

use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::dispatcher::ServiceStatus,
    http::{
        error::HttpResult,
        state::HttpState,
        types::app::{App, ShaDigest, Status},
    },
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAppsResponse {
    pub apps: Vec<App>,
    pub digests: Vec<ShaDigest>,
}

#[axum::debug_handler]
pub async fn handle_list_services(State(state): State<HttpState>) -> impl IntoResponse {
    match list_services_inner(&state).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn list_services_inner(state: &HttpState) -> HttpResult<ListAppsResponse> {
    let services = state
        .dispatcher
        .list_services(Bound::Unbounded, Bound::Unbounded)?;

    let mut apps = Vec::with_capacity(services.len());

    // for backwards compatibility, we do some funky things here
    // it will be nicer in 0.3
    for service in services {
        for component in service.components.values() {
            let digest = component.wasm.clone();
            let envs = component
                .env
                .iter()
                .map(|e| (e[0].clone(), e[1].clone()))
                .collect();
            let permissions = component.permissions.clone();
            let status = match service.status {
                ServiceStatus::Active => Status::Active,
                ServiceStatus::Stopped => Status::Failed,
            };

            let app = App {
                digest: digest.into(),
                envs,
                permissions,
                status: Some(status),
                name: service.id.to_string(),
                // just first workflow for now
                trigger: match service.workflows.values().next() {
                    None => return Err(anyhow::anyhow!("No workflows found").into()),
                    Some(w) => w.trigger.clone(),
                },
                testable: Some(service.testable),
            };

            apps.push(app);
        }
    }

    let digests = state.dispatcher.list_component_digests()?;
    let digests = digests.into_iter().map(Into::into).collect();

    Ok(ListAppsResponse { apps, digests })
}
