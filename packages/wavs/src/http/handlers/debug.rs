use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use wavs_types::{ServiceId, Trigger, TriggerAction, TriggerConfig, TriggerData, WorkflowId};

use crate::http::state::HttpState;

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
) -> StatusCode {
    if !state.config.debug_endpoints_enabled {
        return StatusCode::SERVICE_UNAVAILABLE;
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
        if let Err(e) = state.dispatcher.trigger_manager.add_trigger(action).await {
            tracing::error!("Failed to add trigger: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    StatusCode::OK
}
