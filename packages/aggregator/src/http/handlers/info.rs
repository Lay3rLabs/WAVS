use crate::http::{error::HttpResult, state::HttpState};
use alloy::providers::Provider;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub address: alloy::primitives::Address,
    pub balance: String,
}

#[axum::debug_handler]
pub async fn handle_info(State(state): State<HttpState>) -> impl IntoResponse {
    match inner_handle_info(state).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn inner_handle_info(state: HttpState) -> HttpResult<InfoResponse> {
    let chain_config = state.config.signing_client().await?;
    let address = chain_config.signer.address();
    let account = chain_config.provider.get_account(address).await?;
    let balance = account.balance;

    Ok(InfoResponse {
        address,
        // TODO: decimals?
        balance: balance.to_string(),
    })
}
