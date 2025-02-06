use std::ops::Bound;

use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{TestAppRequest, TestAppResponse, TriggerAction, TriggerConfig};

use crate::http::{error::HttpResult, state::HttpState};

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
        data: req.input,
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    std::thread::spawn({
        let state = state.clone();
        move || {
            let resp = state.dispatcher.run_trigger(action);

            tx.send(resp).unwrap();
        }
    });

    let chain_message = rx.await.unwrap()?;

    let output = serde_json::from_slice(&chain_message.wasi_result)?;

    let resp = TestAppResponse { output };

    Ok(resp)
}
