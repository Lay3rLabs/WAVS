use alloy_primitives::U256;
use alloy_provider::Provider;
use alloy_rpc_types_eth::SyncStatus;
use thiserror::Error;
use wavs_types::ChainName;

use crate::{
    config::{
        ChainConfigs, CosmosChainConfig, EvmChainConfig, EvmChainConfigExt,
    },
    error::EvmClientError,
    evm_client::EvmQueryClient,
};
use wavs_types::AnyChainConfig;

pub async fn health_check_chains_query<'a>(
    chain_configs: &ChainConfigs,
    chain_names: &'a [ChainName],
) -> Result<'a, ()> {
    for chain_name in chain_names {
        let chain = chain_configs.get_chain(chain_name).unwrap().unwrap();

        match chain {
            AnyChainConfig::Evm(config) => {
                check_evm_chain_health_query(chain_name, config).await?;
                tracing::info!("Evm chain [{chain_name}] is healthy");
            }
            AnyChainConfig::Cosmos(config) => {
                check_cosmos_chain_health_query(chain_name, config).await?;
                tracing::info!("Cosmos chain [{chain_name}] is healthy");
            }
        }
    }
    Ok(())
}

async fn check_evm_chain_health_query(
    chain_name: &ChainName,
    config: EvmChainConfig,
) -> Result<()> {
    let endpoint = config
        .query_client_endpoint()
        .map_err(|e| HealthCheckError::EvmClientError(chain_name, e))?;
    let client = EvmQueryClient::new(endpoint)
        .await
        .map_err(|e| HealthCheckError::EvmClientError(chain_name, e))?;

    // Check block number
    client
        .provider
        .get_block_number()
        .await
        .map_err(|e| HealthCheckError::EvmBlockNumber(chain_name, e.to_string()))?;

    // Check chain ID
    client
        .provider
        .get_chain_id()
        .await
        .map_err(|e| HealthCheckError::EvmChainId(chain_name, e.to_string()))?;

    // Check gas price
    client
        .provider
        .get_gas_price()
        .await
        .map_err(|e| HealthCheckError::EvmGasPrice(chain_name, e.to_string()))?;

    // Check if the node is syncing
    let syncing_status = client
        .provider
        .syncing()
        .await
        .map_err(|e| HealthCheckError::EvmSyncingStatus(chain_name, e.to_string()))?;
    if let SyncStatus::Info(sync_info) = syncing_status {
        return Err(HealthCheckError::EvmStillSyncing(
            chain_name,
            sync_info.current_block,
        ));
    }

    Ok(())
}

async fn check_cosmos_chain_health_query(
    chain_name: &ChainName,
    config: CosmosChainConfig,
) -> Result<()> {
    let client = layer_climb::querier::QueryClient::new(config.to_chain_config(), None)
        .await
        .map_err(|e| HealthCheckError::CosmosCreateClient(chain_name, e))?;

    // Check block height
    client
        .block_height()
        .await
        .map_err(|e| HealthCheckError::CosmosBlockHeight(chain_name, e))?;

    // Check node info
    client
        .node_info()
        .await
        .map_err(|e| HealthCheckError::CosmosNodeInfo(chain_name, e))?;

    Ok(())
}

type Result<'a, T> = std::result::Result<T, HealthCheckError<'a>>;

#[derive(Error, Debug)]
pub enum HealthCheckError<'a> {
    #[error("[evm.{0}] {1:?}")]
    EvmClientError(&'a ChainName, EvmClientError),

    #[error("[evm.{0}] Failed to get block number: {1}")]
    EvmBlockNumber(&'a ChainName, String),

    #[error("[evm.{0}] Failed to get chain ID: {1}")]
    EvmChainId(&'a ChainName, String),

    #[error("[evm.{0}] Failed to get gas price: {1}")]
    EvmGasPrice(&'a ChainName, String),

    #[error("[evm.{0}] Failed to get syncing status: {1}")]
    EvmSyncingStatus(&'a ChainName, String),

    #[error("[evm.{0}] Chain is still syncing: {1}")]
    EvmStillSyncing(&'a ChainName, U256),

    #[error("[cosmos.{0}] create client: {1:?}")]
    CosmosCreateClient(&'a ChainName, anyhow::Error),

    #[error("[cosmos.{0}] block height: {1:?}")]
    CosmosBlockHeight(&'a ChainName, anyhow::Error),

    #[error("[cosmos.{0}] node info: {1:?}")]
    CosmosNodeInfo(&'a ChainName, anyhow::Error),
}
