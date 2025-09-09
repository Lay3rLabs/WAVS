use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use wavs_types::{ServiceId, Trigger, TriggerAction, TriggerConfig, TriggerData, WorkflowId};

use crate::http::{error::HttpResult, state::HttpState};

#[derive(Debug, Deserialize)]
pub struct SimulatedTriggerRequest {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger: Trigger,
    pub data: TriggerData,
    #[serde(default = "default_count")]
    pub count: usize,
}

fn default_count() -> usize {
    1
}

pub async fn handle_debug_trigger(
    State(state): State<HttpState>,
    Json(req): Json<SimulatedTriggerRequest>,
) -> impl IntoResponse {
    match debug_trigger_inner(state, req).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn debug_trigger_inner(state: HttpState, req: SimulatedTriggerRequest) -> HttpResult<()> {
    if !state.config.debug_endpoints_enabled {
        return Err(anyhow::anyhow!("Debug endpoints are not enabled").into());
    }

    for _ in 0..req.count {
        let action = TriggerAction {
            config: TriggerConfig {
                service_id: req.service_id.clone(),
                workflow_id: req.workflow_id.clone(),
                trigger: req.trigger.clone(),
            },
            data: req.data.clone(),
        };

        state
            .dispatcher
            .trigger_manager
            .add_trigger(action)
            .await
            .map_err(|e| {
                tracing::error!("Failed to add trigger: {}", e);
                anyhow::anyhow!("Failed to add trigger: {}", e)
            })?;
    }

    Ok(())
}
