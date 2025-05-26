use crate::http::{error::HttpResult, state::HttpState};
use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct InfoResponse {
    pub operators: Vec<String>,
}

#[utoipa::path(
    get,
    path = "/info",
    responses(
        (status = 200, description = "Successfully retrieved service information including the list of active operators", body = InfoResponse),
        (status = 500, description = "Internal server error occurred while fetching service information")
    ),
    description = "Provides information about the WAVS service, including a list of all registered operators."
)]
#[axum::debug_handler]
pub async fn handle_info(State(state): State<HttpState>) -> impl IntoResponse {
    match inner_handle_info(state).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn inner_handle_info(state: HttpState) -> HttpResult<InfoResponse> {
    // TODO - get the operators from the dispatcher, and/or Eigenlayer?

    let cosmos_chain_config: layer_climb::prelude::ChainConfig = state
        .config
        .chains
        .cosmos
        .values()
        .next()
        .context("no active cosmos chain")?
        .clone()
        .into();

    let mnemonic = state
        .config
        .cosmos_submission_mnemonic
        .clone()
        .context("submission_mnemonic not set")?;

    let mut operators = Vec::new();

    let climb_address_kind = cosmos_chain_config.address_kind;

    for i in 0..10 {
        let key_signer =
            KeySigner::new_mnemonic_str(&mnemonic, Some(&cosmos_hub_derivation(i)?)).unwrap();
        let address = climb_address_kind.address_from_pub_key(&key_signer.public_key().await?)?;
        operators.push(address.to_string());
    }

    Ok(InfoResponse { operators })
}
