use alloy::sol_types::SolCall;
use axum::{extract::State, response::IntoResponse, Json};
use utils::{
    eth_client::{AddTaskRequest, AddTaskResponse},
    hello_world::solidity_types::hello_world::HelloWorldServiceManager,
};

use crate::http::{
    error::HttpResult,
    state::{HttpState, Task},
};

#[axum::debug_handler]
pub async fn handle_add_message(
    State(state): State<HttpState>,
    Json(req): Json<AddTaskRequest>,
) -> impl IntoResponse {
    match add_task(state, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn add_task(state: HttpState, req: AddTaskRequest) -> HttpResult<AddTaskResponse> {
    let task = Task::new(req.operator, req.new_data, req.signature);
    let key = (req.task_id, req.service);
    let mut tasks = state.load(&key);
    tasks.push(task);

    if tasks.len() >= state.config.tasks_quorum as usize {
        let eth_client = state.config.signing_client().await?;
        // TODO: decide how to batch it. It seems like a complex topic with many options
        // Current options require something from node, extra contract dependency or uncertainty
        // Options:
        // - Use `Multicall` contract, tracking issue: https://github.com/alloy-rs/alloy/issues/328,
        //      non-official implementation: https://crates.io/crates/alloy-multicall (there is no send, how to even use it)
        // - trace_call_many, check note: https://docs.rs/alloy/0.8.0/alloy/providers/ext/trait.TraceApi.html#tymethod.trace_call_many
        // - Use eip-7702, doubt it's possible to do exactly what we're trying to achieve here

        // Send "batch" txs
        let hello_world_service =
            HelloWorldServiceManager::new(key.1.clone(), &eth_client.http_provider);
        let mut txs = Vec::with_capacity(tasks.len());
        for task in tasks {
            let mut call =
                HelloWorldServiceManager::respondToTaskCall::abi_decode(&task.data, true)?;
            call.signature = task.signature.clone().into();
            let pending_tx = hello_world_service.call_builder(&call).send().await?;
            txs.push(pending_tx);
        }
        let tx_hashes: Vec<_> = txs.iter().map(|tx| tx.tx_hash().to_owned()).collect();
        tracing::debug!("Sent transactions: {tx_hashes:?}");

        let mut last_tx = None;
        for pending_tx in txs {
            last_tx = Some(pending_tx.watch().await?);
        }
        tracing::debug!("Transactions included in a block");
        state.remove(&key);
        Ok(AddTaskResponse { hash: last_tx })
    } else {
        state.save(key, tasks);
        Ok(AddTaskResponse { hash: None })
    }
}

// // Followup issue, this check is against a local DB, registered via endpoint
// check_if_operator(lookup_id, operator_address, new_signature);
