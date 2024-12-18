use std::ops::Bound;

use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use lavs_apis::id::TaskId;
use serde::{Deserialize, Serialize};

use crate::{
    apis::trigger::{Trigger, TriggerAction, TriggerConfig, TriggerData},
    http::{error::HttpResult, state::HttpState},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAppRequest {
    pub name: String,
    pub input: Option<serde_json::Value>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestAppResponse {
    pub output: serde_json::Value,
}

#[axum::debug_handler]
pub async fn handle_test_service(
    State(state): State<HttpState>,
    Json(req): Json<TestAppRequest>,
) -> impl IntoResponse {
    let resp = test_service_inner(&state, req).await;
    match resp {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn test_service_inner(state: &HttpState, req: TestAppRequest) -> HttpResult<TestAppResponse> {
    let input = req.input.unwrap_or_default();

    let services = state
        .dispatcher
        .list_services(Bound::Included(&req.name), Bound::Included(&req.name))?;

    let service = services
        .first()
        .context(format!("Service {} not found", req.name))?;

    // TODO: just use the first workflow for now
    let (workflow_id, workflow) = service
        .workflows
        .iter()
        .next()
        .context("No workflows found")?;

    let action = TriggerAction {
        config: TriggerConfig {
            service_id: service.id.clone(),
            workflow_id: workflow_id.clone(),
            trigger: workflow.trigger.clone(),
        },
        data: match workflow.trigger {
            Trigger::LayerQueue { .. } => {
                TriggerData::queue(TaskId::new(0), serde_json::to_vec(&input)?.as_slice())
            }
            Trigger::EthEvent { .. } => {
                TriggerData::queue(TaskId::new(0), serde_json::to_vec(&input)?.as_slice())
            }
        },
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    std::thread::spawn({
        let state = state.clone();
        move || {
            let resp = state.dispatcher.run_trigger(action);

            tx.send(resp).unwrap();
        }
    });

    let chain_message = rx.await.unwrap()?.context("could not get chain message")?;

    let output = serde_json::from_slice(&chain_message.wasm_result)?;

    let resp = TestAppResponse { output };

    Ok(resp)
}
