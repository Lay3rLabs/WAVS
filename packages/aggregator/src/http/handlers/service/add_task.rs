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
    let mut task = Task {
        signatures: HashMap::new(),
        operators: req.operators,
        avl: req.avl,
        reference_block: req.reference_block,
        function: req.function,
        input: req.input,
        erc1271: req.erc1271,
    };

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
