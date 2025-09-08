use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::instrument;
use utoipa::ToSchema;
use wavs_types::{ServiceID, Trigger, TriggerAction, TriggerConfig, TriggerData, WorkflowID};

use crate::{dispatcher::DispatcherCommand, http::state::HttpState};

#[derive(Deserialize, ToSchema)]
pub struct SimulatedTriggerRequest {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub trigger: Trigger,
    pub data: Option<TriggerData>,
    #[serde(default = "default_count")]
    pub count: usize,
    #[serde(default)]
    pub delay_ms: u64,
}

fn default_count() -> usize {
    1
}

#[derive(Serialize, ToSchema)]
pub struct DebugTriggerResponse {
    pub message: String,
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub count: usize,
}

#[utoipa::path(
    post,
    path = "/debug/trigger",
    request_body = SimulatedTriggerRequest,
    responses(
        (status = 200, description = "Triggers queued successfully", body = DebugTriggerResponse),
        (status = 400, description = "Invalid service or workflow"),
        (status = 503, description = "Debug endpoints disabled"),
    ),
)]
#[instrument(level = "debug", skip(state), fields(service_id = %req.service_id, workflow_id = %req.workflow_id))]
pub async fn handle_debug_trigger(
    State(state): State<HttpState>,
    Json(req): Json<SimulatedTriggerRequest>,
) -> Result<Json<DebugTriggerResponse>, StatusCode> {
    if !state.config.debug_endpoints_enabled {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let service = state
        .services
        .get(&req.service_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if !service.workflows.contains_key(&req.workflow_id) {
        return Err(StatusCode::BAD_REQUEST);
    }

    tracing::info!(
        "Debug trigger injection: {} triggers for service {} workflow {}",
        req.count,
        req.service_id,
        req.workflow_id
    );

    let mut trigger_actions = Vec::new();
    for i in 0..req.count {
        let data = req
            .data
            .clone()
            .unwrap_or_else(|| generate_simulated_trigger_data(&req.trigger, i));

        let action = TriggerAction {
            config: TriggerConfig {
                service_id: req.service_id.clone(),
                workflow_id: req.workflow_id.clone(),
                trigger: req.trigger.clone(),
            },
            data,
        };

        trigger_actions.push(action);
    }

    let dispatcher_tx = state.dispatcher_tx.clone();
    let count = req.count;
    let delay_ms = req.delay_ms;

    tokio::spawn(async move {
        for (i, action) in trigger_actions.into_iter().enumerate() {
            if let Err(e) = dispatcher_tx.send(DispatcherCommand::Trigger(action)).await {
                tracing::error!("Failed to send debug trigger {}/{}: {}", i + 1, count, e);
                break;
            }

            if delay_ms > 0 && i < count - 1 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
        tracing::info!("Completed sending {} debug triggers", count);
    });

    Ok(Json(DebugTriggerResponse {
        message: format!(
            "Queued {} trigger(s) with {}ms delay",
            req.count, req.delay_ms
        ),
        service_id: req.service_id,
        workflow_id: req.workflow_id,
        count: req.count,
    }))
}

fn generate_simulated_trigger_data(trigger: &Trigger, index: usize) -> TriggerData {
    match trigger {
        Trigger::EvmContractEvent { .. } => {
            let block_number = 1000000 + index as u64;
            let block_hash = format!("0x{:064x}", index);
            let tx_hash = format!("0x{:064x}", index * 2);

            TriggerData::EvmContractEvent {
                block_number,
                block_hash: block_hash.parse().unwrap(),
                transaction_hash: tx_hash.parse().unwrap(),
                log_index: index as u64,
                topics: vec![],
                data: vec![],
            }
        }
        Trigger::CosmosContractEvent { .. } => TriggerData::CosmosContractEvent {
            height: 1000000 + index as u64,
            tx_hash: format!("{:064x}", index),
            event_index: index as u64,
            attributes: vec![],
        },
        Trigger::Block { .. } => {
            let height = 1000000 + index as u64;
            let hash = format!("0x{:064x}", index);

            TriggerData::Block {
                height,
                hash: hash.parse().unwrap(),
                timestamp: chrono::Utc::now().timestamp() as u64,
            }
        }
        Trigger::Cron { .. } => TriggerData::Cron {
            timestamp: chrono::Utc::now().timestamp() as u64,
        },
        Trigger::Never => TriggerData::Never,
    }
}
