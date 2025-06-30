use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utils::config::AnyChainConfig;
use utoipa::ToSchema;
use wavs_types::ChainName;

use crate::http::{error::HttpResult, state::HttpState};

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub struct AddChainRequest {
    pub chain_name: ChainName,
    pub chain_config: AnyChainConfig,
}

#[utoipa::path(
    post,
    path = "/add-chain",
    request_body = AddChainRequest,
    responses(
        (status = 200, description = "Chain added successfully"),
        (status = 400, description = "Invalid chain config"),
        (status = 409, description = "Chain already exists"),
        (status = 500, description = "Internal server error")
    ),
    description = "Dynamically adds a new chain configuration"
)]
#[axum::debug_handler]
pub async fn handle_add_chain(
    State(state): State<HttpState>,
    Json(request): Json<AddChainRequest>,
) -> impl IntoResponse {
    match add_chain_inner(state, request.chain_name, request.chain_config).await {
        Ok(_) => axum::http::StatusCode::OK.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_chain_inner(
    state: HttpState,
    chain_name: ChainName,
    chain_config: AnyChainConfig,
) -> HttpResult<()> {
    state.dispatcher.add_chain(chain_name, chain_config)?;
    Ok(())
}
