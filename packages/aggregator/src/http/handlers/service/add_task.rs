use alloy::{
    primitives::{keccak256, Bytes, PrimitiveSignature, U256},
    sol_types::SolValue,
};
use anyhow::anyhow;
use axum::{extract::State, response::IntoResponse, Json};
use utils::{
    eigen_client::solidity_types::{
        misc::{AVSDirectory, IAVSDirectory::OperatorAVSRegistrationStatus},
        HttpSigningProvider,
    },
    hello_world::{
        solidity_types::{
            hello_world::{self, HelloWorldServiceManager},
            stake_registry::ECDSAStakeRegistry,
        },
        AddTaskRequest, AddTaskResponse, HelloWorldSimpleClient,
    },
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
    let eth_client = state.config.signing_client().await?;

    check_operator(&req, &eth_client.http_provider).await?;
    let task = Task::new(
        req.operator,
        req.new_data,
        req.signature,
        req.reference_block,
    );
    let mut tasks_map = state.load_tasks(req.service)?;
    let queue = tasks_map
        .entry(req.task_id)
        .or_insert_with(Default::default);
    queue.push(task);

    let hash = if queue.len() >= state.config.tasks_quorum as usize {
        // TODO: decide how to batch it. It seems like a complex topic with many options
        // Current options require something from node, extra contract dependency or uncertainty
        // Options:
        // - Use `Multicall` contract, tracking issue: https://github.com/alloy-rs/alloy/issues/328,
        //      non-official implementation: https://crates.io/crates/alloy-multicall (there is no send, how to even use it)
        // - trace_call_many, check note: https://docs.rs/alloy/0.8.0/alloy/providers/ext/trait.TraceApi.html#tymethod.trace_call_many
        // - Use eip-7702, doubt it's possible to do exactly what we're trying to achieve here
        // ✅(currently implemented) Add respond many on AVS endpoint

        // Send batch txs
        let hello_world_service =
            hello_world::HelloWorldServiceManager::new(req.service, &eth_client.http_provider);
        let mut tasks = vec![];
        let mut indexes = vec![];
        let mut signatures = vec![];
        for item in queue.drain(..) {
            let task_data = item.data;
            tasks.push(hello_world::Task {
                name: task_data.name,
                taskCreatedBlock: task_data.task_created_block,
            });
            indexes.push(task_data.task_index);
            let signature = HelloWorldSimpleClient::batch_signature(
                item.signature,
                item.operator,
                item.reference_block,
            );

            signatures.push(Bytes::from(signature));
        }

        let pending_tx = hello_world_service
            .respondToTasks(tasks, indexes, signatures)
            .send()
            .await?;
        let tx_hash = pending_tx.tx_hash();
        tracing::debug!("Sent transaction: {}", tx_hash);

        let tx_hash = pending_tx.watch().await?;
        tracing::debug!("Transactions included in a block");
        Some(tx_hash)
    } else {
        None
    };
    state.save_tasks(req.service, tasks_map)?;
    Ok(AddTaskResponse { hash })
}

pub async fn check_operator(
    task_request: &AddTaskRequest,
    provider: &HttpSigningProvider,
) -> HttpResult<()> {
    let service = task_request.service;
    let operator = task_request.operator;

    let hello_world_service = HelloWorldServiceManager::new(service, provider);

    // Check Operator is registered
    let avs_directory_address = hello_world_service.avsDirectory().call().await?._0;
    let avs_directory = AVSDirectory::new(avs_directory_address, provider);
    let operator_status = avs_directory
        .avsOperatorStatus(service, operator)
        .call()
        .await?
        ._0;
    if operator_status != OperatorAVSRegistrationStatus::REGISTERED().into() {
        return Err(anyhow!("Operator is not registered {operator} in {service}").into());
    }

    // Check operator signature matches
    let stake_registry = hello_world_service.stakeRegistry().call().await?._0;
    let ecdsa_signature = ECDSAStakeRegistry::new(stake_registry, provider);
    let expected_address = ecdsa_signature
        .getOperatorSigningKeyAtBlock(operator, U256::from(task_request.reference_block))
        .call()
        .await?
        ._0;
    let msg = keccak256(
        format!("Hello, {}", task_request.new_data.name)
            .abi_encode_packed()
            .as_slice(),
    );
    let signature_address = PrimitiveSignature::try_from(task_request.signature.as_slice())?
        .recover_address_from_msg(msg)?;
    if expected_address != signature_address {
        return Err(anyhow!("Operator signature does not match").into());
    }

    Ok(())
}
