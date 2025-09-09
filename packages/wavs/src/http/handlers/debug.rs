use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use wavs_types::{SimulatedTriggerRequest, TriggerAction, TriggerConfig};

use crate::http::{error::HttpResult, state::HttpState};

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
