use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utils::config::ChainConfigs;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InfoResponse {
    pub chains: ChainConfigs,
}

#[axum::debug_handler]
pub async fn handle_info(State(state): State<HttpState>) -> impl IntoResponse {
    match inner_handle_info(state).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn inner_handle_info(state: HttpState) -> HttpResult<InfoResponse> {
    Ok(InfoResponse {
        chains: state.config.chains,
    })
}
