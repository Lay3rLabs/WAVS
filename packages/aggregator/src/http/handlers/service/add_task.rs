use std::{collections::HashMap, iter};

use alloy::{
    contract::{ContractInstance, Interface},
    dyn_abi::{DynSolValue, JsonAbiExt},
    json_abi::JsonAbi,
    primitives::{eip191_hash_message, keccak256, Address, TxHash, U256},
    sol_types::SolCall,
};
use anyhow::{bail, ensure, Context};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utils::eth_client::{AddTaskRequest, OperatorSignature};

use crate::{
    http::{
        error::HttpResult,
        state::{HttpState, Task},
    },
    solidity_types::erc1271::IERC1271::{self, IERC1271Instance},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddMessageResponse {
    pub submitted: Option<TxHash>,
}

#[axum::debug_handler]
pub async fn handle_add_message(
    State(state): State<HttpState>,
    Json(req): Json<AddTaskRequest>,
) -> impl IntoResponse {
    match add_message(state, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn add_message(state: HttpState, req: AddTaskRequest) -> HttpResult<AddMessageResponse> {
    let mut aggregator_state = state.aggregator_state.write().unwrap();
    if aggregator_state.contains_key(&req.task_name) {
        return Err(anyhow::anyhow!("Task already exists").into());
    }
    let mut task = Task {
        signatures: HashMap::new(),
        operators: req.operators,
        avl: req.avl,
        reference_block: req.reference_block,
        function: req.function,
        input: req.input,
        erc1271: req.erc1271,
    };
    let client = state.config.signing_client().await?;

    add_signature(&mut task, req.signature);
    let erc1271 = IERC1271Instance::new(task.erc1271, &client.http_provider);
    let hash = eip191_hash_message(keccak256(req.task_name));
    let signature_bytes = signature_bytes(task.signatures.clone(), task.reference_block);

    // Check if we have enough weight
    // TODO: what if we get invalid signatures?
    if let Ok(valid_signature) = erc1271
        .isValidSignature(hash, signature_bytes.clone().into())
        .await
    {
        if valid_signature.magicValue == IERC1271::isValidSignatureCall::SELECTOR {
            let avl_contract = ContractInstance::new(
                task.avl,
                client.http_provider,
                Interface::new(JsonAbi::from_iter(iter::once(task.function.clone().into()))),
            );

            aggregator_state.insert(req.task_name, task);
            // Searching signature param index
            let signature_index = req
                .function
                .inputs
                .iter()
                .enumerate()
                .find_map(|(idx, param)| param.name.eq("signature").then(|| idx))
                .context("signature")?;
            let mut args = req.function.abi_decode_input(&req.input, false)?;
            let DynSolValue::Bytes(bytes) = &mut args[signature_index] else {
                return Err(anyhow::anyhow!("Signature supposed to be bytes").into());
            };
            *bytes = signature_bytes;

            let receipt = avl_contract
                .function(&req.function.name, &args)?
                .gas(500000)
                .send()
                .await?
                .get_receipt()
                .await?;

            if !receipt.status() {
                return Err(anyhow::anyhow!("Failed to submit task").into());
            }
        }
    }
    Ok(AddMessageResponse { submitted: None })
}

pub fn add_signature(task: &mut Task, signature: OperatorSignature) -> anyhow::Result<()> {
    let OperatorSignature { address, signature } = signature;
    ensure!(
        !task.operators.contains(&address),
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
