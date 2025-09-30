use alloy_primitives::U256;
use alloy_provider::Provider;
use alloy_rpc_types_eth::SyncStatus;
use thiserror::Error;

use crate::{
    config::{CosmosChainConfig, EvmChainConfig, EvmChainConfigExt},
    error::EvmClientError,
    evm_client::EvmQueryClient,
};
use wavs_types::{AnyChainConfig, ChainKey};

pub async fn health_check_single_chain(
    key: &ChainKey,
    config: &AnyChainConfig,
) -> anyhow::Result<()> {
    match config {
        AnyChainConfig::Evm(config) => {
            check_evm_chain_health_query(key.clone(), config.clone()).await?;
            tracing::info!("Evm chain [{key}] is healthy");
        }
        AnyChainConfig::Cosmos(config) => {
            check_cosmos_chain_health_query(key.clone(), config.clone()).await?;
            tracing::info!("Cosmos chain [{key}] is healthy");
        }
    }
    Ok(())
}

async fn check_evm_chain_health_query(
    key: ChainKey,
    config: EvmChainConfig,
) -> Result<(), HealthCheckError> {
    let endpoint = config
        .query_client_endpoint()
        .map_err(|e| HealthCheckError::EvmClientError(key.clone(), e))?;
    let client = EvmQueryClient::new(endpoint)
        .await
        .map_err(|e| HealthCheckError::EvmClientError(key.clone(), e))?;

    // Check block number
    client
        .provider
        .get_block_number()
        .await
        .map_err(|e| HealthCheckError::EvmBlockNumber(key.clone(), e.to_string()))?;

    // Check chain ID
    client
        .provider
        .get_chain_id()
        .await
        .map_err(|e| HealthCheckError::EvmChainId(key.clone(), e.to_string()))?;

    // Check gas price
    client
        .provider
        .get_gas_price()
        .await
        .map_err(|e| HealthCheckError::EvmGasPrice(key.clone(), e.to_string()))?;

    // Check if the node is syncing
    let syncing_status = client
        .provider
        .syncing()
        .await
        .map_err(|e| HealthCheckError::EvmSyncingStatus(key.clone(), e.to_string()))?;
    if let SyncStatus::Info(sync_info) = syncing_status {
        return Err(HealthCheckError::EvmStillSyncing(
            key,
            sync_info.current_block,
        ));
    }

    Ok(())
}

async fn check_cosmos_chain_health_query(
    key: ChainKey,
    config: CosmosChainConfig,
) -> Result<(), HealthCheckError> {
    let client = layer_climb::querier::QueryClient::new(config.to_chain_config(), None)
        .await
        .map_err(|e| HealthCheckError::CosmosCreateClient(key.clone(), e))?;

    // Check block height
    client
        .block_height()
        .await
        .map_err(|e| HealthCheckError::CosmosBlockHeight(key.clone(), e))?;

    // Check node info
    client
        .node_info()
        .await
        .map_err(|e| HealthCheckError::CosmosNodeInfo(key.clone(), e))?;

    Ok(())
}

#[derive(Error, Debug)]
pub enum HealthCheckError {
    #[error("[{0}] {1:?}")]
    EvmClientError(ChainKey, EvmClientError),

    #[error("[{0}] Failed to get block number: {1}")]
    EvmBlockNumber(ChainKey, String),

    #[error("[{0}] Failed to get chain ID: {1}")]
    EvmChainId(ChainKey, String),

    #[error("[{0}] Failed to get gas price: {1}")]
    EvmGasPrice(ChainKey, String),

    #[error("[{0}] Failed to get syncing status: {1}")]
    EvmSyncingStatus(ChainKey, String),

    #[error("[{0}] Chain is still syncing: {1}")]
    EvmStillSyncing(ChainKey, U256),

    #[error("[{0}] create client: {1:?}")]
    CosmosCreateClient(ChainKey, anyhow::Error),

    #[error("[{0}] block height: {1:?}")]
    CosmosBlockHeight(ChainKey, anyhow::Error),

    #[error("[{0}] node info: {1:?}")]
    CosmosNodeInfo(ChainKey, anyhow::Error),
}
