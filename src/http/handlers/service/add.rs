use std::collections::BTreeMap;

use axum::{extract::State, response::IntoResponse, Json};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    apis::{
        dispatcher::{Component, Service, ServiceStatus, Submit, Workflow},
        Trigger, ID,
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

    let submit = match &req.app.trigger {
        Trigger::Queue {
            task_queue_addr, ..
        } => {
            let hd_index = 0; // TODO: should this come from the request?

            let query_client = QueryClient::new(state.config.chain_config()?).await?;
            let task_queue_addr = query_client.chain_config.parse_address(task_queue_addr)?;

            let resp: lavs_apis::tasks::ConfigResponse = query_client
                .contract_smart(
                    &task_queue_addr,
                    &lavs_apis::tasks::QueryMsg::Custom(
                        lavs_apis::tasks::CustomQueryMsg::Config {},
                    ),
                )
                .await?;

            Some(Submit::verifier_tx(hd_index, &resp.verifier))
        }
    };

    let workflows = BTreeMap::from([(
        workflow_id,
        Workflow {
            trigger: req.app.trigger,
            component: component_id,
            submit,
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
