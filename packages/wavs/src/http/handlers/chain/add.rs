use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{AddChainRequest, AnyChainConfig, ChainKey};

use crate::{
    http::{error::HttpResult, state::HttpState},
    subsystems::{engine::EngineCommand, trigger::TriggerCommand},
};

#[utoipa::path(
    post,
    path = "/dev/chains",
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
    match add_chain_inner(state, request.chain, request.config).await {
        Ok(_) => axum::http::StatusCode::OK.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_chain_inner(
    state: HttpState,
    chain: ChainKey,
    config: AnyChainConfig,
) -> HttpResult<()> {
    // Notify trigger manager about the new chain
    state
        .dispatcher
        .trigger_manager
        .command_sender
        .send(TriggerCommand::AddChain {
            chain: chain.clone(),
            config: config.clone(),
        })
        .map_err(|_| anyhow::anyhow!("Failed to notify trigger manager"))?;

    // Notify engine manager about the new chain
    state
        .dispatcher
        .dispatcher_to_engine_tx
        .send(EngineCommand::AddChain {
            chain: chain.clone(),
            config: config.clone(),
        })
        .map_err(|_| anyhow::anyhow!("Failed to notify engine manager"))?;

    // Update dispatcher's chain configs
    state
        .dispatcher
        .chain_configs
        .write()
        .map_err(|_| anyhow::anyhow!("Chain configs lock is poisoned"))?
        .add_chain(chain, config)?;

    Ok(())
}
