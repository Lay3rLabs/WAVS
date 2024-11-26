use crate::http::{error::HttpResult, state::HttpState};
use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub operators: Vec<String>,
}

#[axum::debug_handler]
pub async fn handle_info(State(state): State<HttpState>) -> impl IntoResponse {
    match inner_handle_info(state).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn inner_handle_info(state: HttpState) -> HttpResult<InfoResponse> {
    // TODO - get the operators from the dispatcher?

    let chain_config = state.config.chain_config()?;
    let mnemonic = state
        .config
        .wavs_chain_config()?
        .submission_mnemonic
        .context("submission_mnemonic not set")?;

    let mut operators = Vec::new();

    for i in 0..10 {
        let key_signer =
            KeySigner::new_mnemonic_str(&mnemonic, Some(&cosmos_hub_derivation(i)?)).unwrap();
        let address = chain_config
            .address_kind
            .address_from_pub_key(&key_signer.public_key().await?)?;
        operators.push(address.to_string());
    }

    Ok(InfoResponse { operators })
}
