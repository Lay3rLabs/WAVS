use std::iter;

use alloy::{
    contract::{ContractInstance, Interface},
    dyn_abi::{DynSolValue, JsonAbiExt},
    json_abi::{Function, JsonAbi},
    primitives::Address,
};
use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::http::{error::HttpResult, state::HttpState};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddMessageRequest {
    #[serde(flatten)]
    pub message: MessageRequest,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MessageRequest {
    // TODO: Maybe redundant
    pub operators: Vec<Address>,
    pub signature: Vec<u8>,
    pub task_name: String,
    pub avl: Address,
    pub function: Function,
    pub function_input: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddMessageResponse {}

#[axum::debug_handler]
pub async fn handle_add_message(
    State(state): State<HttpState>,
    Json(req): Json<AddMessageRequest>,
) -> impl IntoResponse {
    match add_message(state, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_message(state: HttpState, req: AddMessageRequest) -> HttpResult<AddMessageResponse> {
    // TODO: add to an actual db. For now we just send it

    let signing_client = state.config.signing_client().await?;
    // Searching signature param index
    let signature_index = req
        .message
        .function
        .inputs
        .iter()
        .enumerate()
        .find_map(|(idx, param)| param.name.eq("signature").then(|| idx))
        .context("signature")?;
    let mut args = req
        .message
        .function
        .abi_decode_input(&req.message.function_input, true)?;
    let DynSolValue::Bytes(bytes) = &mut args[signature_index] else {
        return Err(anyhow::anyhow!("Signature supposed to be bytes").into());
    };
    bytes.copy_from_slice(&req.message.signature);
    let avl = ContractInstance::new(
        req.message.avl,
        signing_client.http_provider,
        Interface::new(JsonAbi::from_iter(iter::once(
            req.message.function.clone().into(),
        ))),
    );
    let receipt = avl
        .function(&req.message.function.name, &args)?
        .gas(500000)
        .send()
        .await?
        .get_receipt()
        .await?;

    tracing::debug!("receipt: {:?}", receipt);
    Ok(AddMessageResponse {})
}
