use crate::http::{error::HttpResult, state::HttpState};
use alloy::providers::Provider;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub signer_address: alloy::primitives::Address,
    pub signer_balance: String,
}

#[axum::debug_handler]
pub async fn handle_info(State(state): State<HttpState>) -> impl IntoResponse {
    match inner_handle_info(state, "local".to_string()).await {
        // TODO: fix this
        Ok(response) => Json(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn inner_handle_info(state: HttpState, chain_name: String) -> HttpResult<InfoResponse> {
    let signing_client = state.config.signing_client(&chain_name).await?;
    let address = signing_client.signer.address();
    let account = signing_client.provider.get_account(address).await?;
    let balance = account.balance;

    Ok(InfoResponse {
        signer_address: address,
        signer_balance: balance.to_string(),
    })
}
