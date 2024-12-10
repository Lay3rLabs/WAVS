use std::{collections::HashMap, iter};

use alloy::{
    contract::{ContractInstance, Interface},
    dyn_abi::{DynSolValue, JsonAbiExt},
    json_abi::JsonAbi,
    primitives::{eip191_hash_message, keccak256, Address, TxHash, U256},
    sol_types::SolCall,
};
use anyhow::{ensure, Context};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utils::eth_client::{AddTaskRequest, AddTaskResponse, OperatorSignature};

use crate::{
    http::{
        error::HttpResult,
        state::{HttpState, Task},
    },
    solidity_types::erc1271::IERC1271::{self, IERC1271Instance},
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

    add_signature(&mut task, req.signature)?;

    let erc1271 = IERC1271Instance::new(
        task.erc1271,
        state.config.signing_client().await?.http_provider.clone(),
    );
    let hash = eip191_hash_message(keccak256(req.task_name.clone()));
    let signature_bytes = signature_bytes(task.signatures.clone(), task.reference_block);

    // Check if we have enough weight
    // TODO: what if we get invalid signatures?
    match erc1271
        .isValidSignature(hash, signature_bytes.clone().into())
        .call()
        .await
    {
        Ok(valid_signature) => {
            if valid_signature.magicValue == IERC1271::isValidSignatureCall::SELECTOR {
                tracing::info!("Got enough signatures, submitting tx");
                let avl_contract = ContractInstance::new(
                    task.avl,
                    state.config.signing_client().await?.http_provider,
                    Interface::new(JsonAbi::from_iter(iter::once(task.function.clone().into()))),
                );

                // Searching signature param index
                let signature_index = task
                    .function
                    .inputs
                    .iter()
                    .enumerate()
                    .find_map(|(idx, param)| param.name.eq("signature").then(|| idx))
                    .context("signature")?;
                let mut args = task.function.abi_decode_input(&task.input, false)?;
                let DynSolValue::Bytes(bytes) = &mut args[signature_index] else {
                    return Err(anyhow::anyhow!("Signature supposed to be bytes").into());
                };
                *bytes = signature_bytes;

                let receipt = avl_contract
                    .function(&task.function.name, &args)?
                    .gas(500000)
                    .send()
                    .await?
                    .get_receipt()
                    .await?;
                // One operator is enough for submission, no need to store task
                return match receipt.status() {
                    true => Ok(AddTaskResponse {
                        hash: Some(receipt.transaction_hash),
                    }),
                    false => Err(anyhow::anyhow!("Failed to submit task").into()),
                };
            } else {
                tracing::info!("Invalid signature(yet?)");
            }
        }
        Err(e) => {
            panic!("{e:?}");
        }
    };

    let mut aggregator_state = state.aggregator_state.write().unwrap();
    if aggregator_state.contains_key(&req.task_name) {
        return Err(anyhow::anyhow!("Task already exists").into());
    }
    aggregator_state.insert(req.task_name.clone(), task);
    Ok(AddTaskResponse { hash: None })
}

pub fn add_signature(task: &mut Task, signature: OperatorSignature) -> anyhow::Result<()> {
    let OperatorSignature { address, signature } = signature;
    ensure!(
        task.operators.contains(&address),
        "Cannot sign not as an operator"
    );
    task.signatures.insert(address, signature);

    Ok(())
}

fn signature_bytes(signatures: HashMap<Address, Vec<u8>>, reference_block: u64) -> Vec<u8> {
    let (operators, signatures): (Vec<_>, Vec<_>) = signatures
        .into_iter()
        .map(|(operator, signature)| {
            (
                DynSolValue::Address(operator),
                DynSolValue::Bytes(signature),
            )
        })
        .unzip();
    DynSolValue::Tuple(vec![
        DynSolValue::Array(operators),
        DynSolValue::Array(signatures),
        DynSolValue::Uint(U256::from(reference_block), 32),
    ])
    .abi_encode_params()
}
