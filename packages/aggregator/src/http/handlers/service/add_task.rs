use std::collections::HashMap;

use axum::{extract::State, response::IntoResponse, Json};
use utils::eth_client::{AddTaskRequest, AddTaskResponse};

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
    let mut task = Task::new(req.operator, req.new_data, req.signature);
    let key = (req.task_id, req.service);
    let tasks = state.load(&key);
    tasks.push(task);
    if tasks.len() > state.config.tasks_for_trigger {}
    task.add_signature(req.signature)?;

    // Try to complete, we need to check signatures and broadcast in case this operator have enough weight to sign by himself
    let provider = state.config.signing_client().await?;
    match task
        .try_completing(&req.task_name, &provider.http_provider)
        .await
    {
        Ok(Some(tx_hash)) => Ok(AddTaskResponse {
            hash: Some(tx_hash),
        }),
        Ok(None) => {
            let mut aggregator_state = state.aggregator_state.write().unwrap();
            if aggregator_state.contains_key(&req.task_name) {
                return Err(anyhow::anyhow!("Task already exists").into());
            }
            aggregator_state.insert(req.task_name.clone(), task);
            Ok(AddTaskResponse { hash: None })
        }
        Err(e) => Err(e.into()),
    }
}

// @dakom reference
// // Operator signs data and sends to aggregator:
// // - task_id
// // - contract_address
// // - operator_address
// // - new_data
// // - new_signature

// let lookup_id = (contract_address, task_id);
// let contract = HelloWorldSimpleClient::new(contract_address).contract;
// let mut stuff:Vec<(operator_address, data, signature)> = Storage::new(lookup_id).load();

// // Followup issue, this check is against a local DB, registered via endpoint
// check_if_operator(lookup_id, operator_address, new_signature);

// stuff.push((operator, new_data, new_signature));

// // Step 1:
// // this should be configurable
// // test with 1 and 3
// //
// // Step 2:
// // this should be precisely the operators registered via endpoint
// if signatures.len() >= quorum_needed(config) {
//     let calls = stuff
//         .iter()
//         .map(|(_operator_address, data, signature)| {
//             contract.respondToTask(data, signature)
//         })
//         .collect::<Vec<_>>();

//     // how to do in alloy??
//     send_batch_transaction(calls).await?;
//     Storage::new(lookup_id).clear();
// } else {
//     Storage::new(lookup_id).save(operator_address, new_data, new_signature);
// }
