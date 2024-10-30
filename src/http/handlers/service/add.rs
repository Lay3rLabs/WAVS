use std::collections::BTreeMap;

use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::{
        dispatcher::{Component, Service, ServiceStatus, Workflow},
        ID,
    },
    http::{
        error::HttpResult,
        state::HttpState,
        types::app::{App, Status},
    },
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppRequest {
    #[serde(flatten)]
    pub app: App,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppResponse {
    pub name: String,
    pub status: Status,
}

#[axum::debug_handler]
pub async fn handle_add_service(
    State(state): State<HttpState>,
    Json(req): Json<RegisterAppRequest>,
) -> impl IntoResponse {
    match add_service_inner(&state, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_service_inner(
    state: &HttpState,
    req: RegisterAppRequest,
) -> HttpResult<RegisterAppResponse> {
    let component_id = ID::new("default")?;
    let workflow_id = ID::new("default")?;
    let service_id = ID::new(&req.app.name)?;

    let components = BTreeMap::from([(component_id.clone(), Component::new(&req.app.digest))]);

    let workflows = BTreeMap::from([(
        workflow_id,
        Workflow {
            trigger: req.app.trigger,
            component: component_id,
            submit: None,
        },
    )]);

    let service = Service {
        id: service_id,
        name: req.app.name.clone(),
        components,
        workflows,
        status: ServiceStatus::Active,
        testable: req.app.testable.unwrap_or(false),
    };

    state.dispatcher.add_service(service)?;

    Ok(RegisterAppResponse {
        name: req.app.name,
        status: Status::Active,
    })
}
