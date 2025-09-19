use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use wavs_types::{SimulatedTriggerRequest, TriggerAction, TriggerConfig};

use crate::http::{error::HttpResult, state::HttpState};

#[utoipa::path(
    post,
    path = "/dev/triggers",
    request_body = SimulatedTriggerRequest,
    responses(
        (status = 200, description = "Trigger sent successfully"),
        (status = 400, description = "Invalid trigger"),
        (status = 404, description = "Not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Sends a simulated trigger to the WAVS system for testing purposes"
)]
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
    let start = std::time::Instant::now();

    let initial_count = state.dispatcher.submission_manager.get_message_count();

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
            .map_err(|e| {
                tracing::error!("Failed to add trigger: {}", e);
                anyhow::anyhow!("Failed to add trigger: {}", e)
            })?;
    }

    if req.wait_for_completion {
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
        let expected = initial_count + req.count as u64;
        loop {
            if state.dispatcher.submission_manager.get_message_count() >= expected {
                let elapsed = start.elapsed();
                state
                    .metrics
                    .record_trigger_simulation_completed(elapsed.as_secs_f64(), req.count);
                break;
            }
            tick.tick().await;
        }
    }
    Ok(())
}
