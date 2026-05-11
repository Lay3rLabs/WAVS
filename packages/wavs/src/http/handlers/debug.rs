use std::collections::HashMap;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use utoipa::ToSchema;

use wavs_types::{
    ByteArray, ChainKey, DevTriggerStreamInfo, DevTriggerStreamSubscriptionKind,
    DevTriggerStreamsInfo, ServiceId, SimulatedTriggerRequest, Trigger, TriggerAction,
    TriggerConfig, TriggerData, WasmResponse, WorkflowId,
};

use crate::http::{
    error::{HttpError, HttpResult},
    state::HttpState,
};

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

    let initial_count = state
        .dispatcher
        .submission_manager
        .metrics
        .get_request_count();

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
            if state
                .dispatcher
                .submission_manager
                .metrics
                .get_request_count()
                >= expected
            {
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

#[utoipa::path(
    get,
    path = "/dev/trigger-streams",
    responses(
        (status = 200, description = "Trigger streams info", body = DevTriggerStreamsInfo),
    ),
    description = "Get health status of chain endpoints"
)]
#[axum::debug_handler]
pub async fn handle_dev_trigger_streams_info(State(state): State<HttpState>) -> impl IntoResponse {
    let chains = state
        .dispatcher
        .trigger_manager
        .evm_controllers
        .read()
        .unwrap()
        .iter()
        .map(|(chain, controller)| {
            (
                chain.clone(),
                DevTriggerStreamInfo {
                    current_endpoint: controller.connection.current_endpoint(),
                    is_connected: controller.subscriptions.is_connected(),
                    any_active_rpcs_in_flight: controller.subscriptions.any_active_rpcs_in_flight(),
                    active_subscriptions: controller
                        .subscriptions
                        .active_subscriptions()
                        .iter()
                        .map(|(id, kind)| {
                            (
                                id.clone(),
                                match kind {
                                    crate::subsystems::trigger::streams::evm_stream::client::SubscriptionKind::NewHeads => {
                                        DevTriggerStreamSubscriptionKind::NewHeads
                                    },
                                    crate::subsystems::trigger::streams::evm_stream::client::SubscriptionKind::Logs { addresses, topics } => {
                                        DevTriggerStreamSubscriptionKind::Logs{
                                            addresses: addresses.iter().map(|a| ByteArray::new(a.into_array())).collect(),
                                            topics: topics.iter().map(|t| ByteArray::new(t.0)).collect(),
                                        }
                                    },
                                    crate::subsystems::trigger::streams::evm_stream::client::SubscriptionKind::NewPendingTransactions => {
                                        DevTriggerStreamSubscriptionKind::NewPendingTransactions
                                    }
                                },
                            )
                        })
                        .collect(),
                },
            )
        })
        .collect::<HashMap<ChainKey, DevTriggerStreamInfo>>();

    let hypercore = state.dispatcher.trigger_manager.hypercore_streams_info();

    Json(DevTriggerStreamsInfo { chains, hypercore }).into_response()
}

// ── POST /dev/execute — synchronous component execution ──────────────────

/// Request body for the synchronous component execution endpoint.
#[derive(Deserialize, ToSchema)]
pub struct ExecuteRequest {
    /// Service ID (64-char hex hash of the ServiceManager)
    pub service_id: ServiceId,
    /// Workflow ID within the service
    pub workflow_id: WorkflowId,
    /// Trigger definition (determines TriggerConfig)
    pub trigger: Trigger,
    /// Trigger data passed to the component
    pub data: TriggerData,
}

#[utoipa::path(
    post,
    path = "/dev/execute",
    request_body = ExecuteRequest,
    responses(
        (status = 200, description = "Component executed successfully", body = Vec<WasmResponse>),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Service or workflow not found"),
        (status = 500, description = "Execution failed")
    ),
    description = "Synchronously execute a component and return the WasmResponse results. \
                   This bypasses the full trigger/aggregator/submission pipeline and calls \
                   the engine directly, returning the raw component output."
)]
pub async fn handle_dev_execute(
    State(state): State<HttpState>,
    Json(req): Json<ExecuteRequest>,
) -> impl IntoResponse {
    match dev_execute_inner(state, req).await {
        Ok(responses) => (StatusCode::OK, Json(responses)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn dev_execute_inner(
    state: HttpState,
    req: ExecuteRequest,
) -> HttpResult<Vec<WasmResponse>> {
    // 1. Look up the service by ID
    let service = state
        .dispatcher
        .services
        .try_get(&req.service_id)
        .map_err(|e| anyhow::anyhow!("service lookup failed: {e}"))?
        .ok_or(HttpError::NotFound)?;

    // 2. Verify the workflow exists in the service
    if !service.workflows.contains_key(&req.workflow_id) {
        return Err(HttpError::NotFound.into());
    }

    // 3. Build the TriggerAction
    let trigger_action = TriggerAction {
        config: TriggerConfig {
            service_id: req.service_id,
            workflow_id: req.workflow_id,
            trigger: req.trigger,
        },
        data: req.data,
    };

    // 4. Execute directly on the engine (bypasses aggregator/submission)
    let responses = state
        .dispatcher
        .engine_manager
        .engine
        .execute_operator_component(service, trigger_action)
        .await
        .map_err(|e| anyhow::anyhow!("component execution failed: {e}"))?;

    Ok(responses)
}
