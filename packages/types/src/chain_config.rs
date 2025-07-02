use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{ChainName, IDError};

#[derive(Debug, thiserror::Error)]
pub enum ChainConfigError {
    #[error("Expected EVM chain")]
    ExpectedEvmChain,
    #[error("Expected Cosmos chain")]
    ExpectedCosmosChain,
    #[error("Duplicate chain name for {0}")]
    DuplicateChainName(ChainName),
    #[error("Invalid chain name: {0}")]
    InvalidChainName(#[from] IDError),
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct CosmosChainConfig {
    pub chain_id: String,
    pub bech32_prefix: String,
    pub rpc_endpoint: Option<String>,
    pub grpc_endpoint: Option<String>,
    pub gas_price: f32,
    pub gas_denom: String,
    pub faucet_endpoint: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct EvmChainConfig {
    pub chain_id: String,
    pub ws_endpoint: Option<String>,
    pub http_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    pub poll_interval_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnyChainConfig {
    Cosmos(CosmosChainConfig),
    Evm(EvmChainConfig),
}

impl From<CosmosChainConfig> for AnyChainConfig {
    fn from(config: CosmosChainConfig) -> Self {
        AnyChainConfig::Cosmos(config)
    }
}

impl From<EvmChainConfig> for AnyChainConfig {
    fn from(config: EvmChainConfig) -> Self {
        AnyChainConfig::Evm(config)
    }
}

impl TryFrom<AnyChainConfig> for CosmosChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Cosmos(config) => Ok(config),
            AnyChainConfig::Evm(_) => Err(ChainConfigError::ExpectedCosmosChain),
        }
    }
}

impl TryFrom<AnyChainConfig> for EvmChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Evm(config) => Ok(config),
            AnyChainConfig::Cosmos(_) => Err(ChainConfigError::ExpectedEvmChain),
        }
    }
}

impl From<CosmosChainConfig> for ChainConfig {
    fn from(config: CosmosChainConfig) -> Self {
        Self {
            chain_id: ChainId::new(config.chain_id),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: None,
            gas_price: config.gas_price,
            gas_denom: config.gas_denom,
            address_kind: AddrKind::Cosmos {
                prefix: config.bech32_prefix,
            },
        }
    }
}

impl TryFrom<ChainConfig> for CosmosChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: ChainConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: config.chain_id.to_string(),
            bech32_prefix: match config.address_kind {
                AddrKind::Cosmos { prefix } => prefix,
                _ => return Err(ChainConfigError::ExpectedCosmosChain),
            },
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            gas_price: config.gas_price,
            gas_denom: config.gas_denom,
            faucet_endpoint: None,
        })
    }
}

impl CosmosChainConfig {
    pub fn to_chain_config(&self) -> ChainConfig {
        self.clone().into()
    }

    pub fn from_chain_config(config: ChainConfig) -> Result<Self, ChainConfigError> {
        config.try_into()
    }
}

impl TryFrom<AnyChainConfig> for ChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> Result<Self, Self::Error> {
        CosmosChainConfig::try_from(config).map(Into::into)
    }
}

impl TryFrom<ChainConfig> for AnyChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: ChainConfig) -> Result<Self, Self::Error> {
        Ok(CosmosChainConfig::try_from(config)?.into())
    }
}

impl AnyChainConfig {
    pub fn to_cosmos_config(&self) -> Result<CosmosChainConfig, ChainConfigError> {
        match self {
            AnyChainConfig::Cosmos(config) => Ok(config.clone()),
            AnyChainConfig::Evm(_) => Err(ChainConfigError::ExpectedCosmosChain),
        }
    }

    pub fn to_evm_config(&self) -> Result<EvmChainConfig, ChainConfigError> {
        match self {
            AnyChainConfig::Evm(config) => Ok(config.clone()),
            AnyChainConfig::Cosmos(_) => Err(ChainConfigError::ExpectedEvmChain),
        }
    }

    pub fn to_layer_climb_config(&self) -> Result<ChainConfig, ChainConfigError> {
        let cosmos_config = self.to_cosmos_config()?;
        Ok(cosmos_config.to_chain_config())
    }

    pub fn from_layer_climb_config(config: ChainConfig) -> Result<Self, ChainConfigError> {
        let cosmos_config = CosmosChainConfig::from_chain_config(config)?;
        Ok(AnyChainConfig::Cosmos(cosmos_config))
    }
}
