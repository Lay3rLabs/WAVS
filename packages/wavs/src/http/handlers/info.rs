use std::ops::Bound;

use crate::{
    http::{error::HttpResult, state::HttpState},
    subsystems::aggregator::p2p::P2pConfig,
};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wavs_types::{ChainKey, P2pStatus};

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct InfoResponse {
    /// Whether an aggregator credential is configured for Cosmos chains
    pub has_aggregator_cosmos: bool,
    /// Whether an aggregator credential is configured for EVM chains
    pub has_aggregator_evm: bool,
    /// P2P networking configuration
    #[schema(value_type = String)]
    pub p2p_config: P2pConfig,
    /// P2P network status
    pub p2p_status: P2pStatus,
    /// Number of services registered on this node
    pub services_count: usize,
    /// Chain keys configured on this node
    #[schema(value_type = Vec<String>)]
    pub chain_keys: Vec<ChainKey>,
}

#[utoipa::path(
    get,
    path = "/info",
    responses(
        (status = 200, description = "Successfully retrieved WAVS node information", body = InfoResponse),
        (status = 500, description = "Internal server error occurred while fetching node information")
    ),
    description = "Provides information about the WAVS node, including aggregator credentials, P2P config, registered services, and configured chains."
)]
#[axum::debug_handler]
pub async fn handle_info(State(state): State<HttpState>) -> impl IntoResponse {
    match inner_handle_info(state).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn inner_handle_info(state: HttpState) -> HttpResult<InfoResponse> {
    // Get services count
    let services_count = state
        .dispatcher
        .services
        .list(Bound::Unbounded, Bound::Unbounded)
        .map(|s| s.len())
        .unwrap_or(0);

    // Get chain keys
    let chain_keys = state
        .config
        .chains
        .read()
        .unwrap()
        .all_chain_keys()
        .unwrap_or_default();

    // Get P2P status
    let mut p2p_status = state.dispatcher.aggregator.get_p2p_status().await;
    // Fill in discovery_mode from config if not already set by the P2P task
    if p2p_status.discovery_mode.is_empty() {
        p2p_status.discovery_mode = match &state.config.p2p {
            P2pConfig::Disabled => "disabled".to_string(),
            P2pConfig::Local { .. } => "local".to_string(),
            P2pConfig::Remote { .. } => "remote".to_string(),
        };
    }

    Ok(InfoResponse {
        has_aggregator_cosmos: state.config.aggregator_cosmos_credential.is_some(),
        has_aggregator_evm: state.config.aggregator_evm_credential.is_some(),
        p2p_config: state.config.p2p.clone(),
        p2p_status,
        services_count,
        chain_keys,
    })
}
