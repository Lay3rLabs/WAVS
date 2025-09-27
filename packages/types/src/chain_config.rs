use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{ChainKey, ChainKeyError, ChainKeyId, ChainKeyNamespace};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChainConfigError {
    #[error("Expected EVM chain")]
    ExpectedEvmChain,
    #[error("Expected Cosmos chain")]
    ExpectedCosmosChain,
    #[error("Invalid chain: {0}")]
    InvalidChainKey(#[from] ChainKeyError),
    #[error("Chain already exists: {0}")]
    DuplicateChain(ChainKey),
    #[error("Namespace for cosmos chain must be {cosmos} or {dev}, got {0}", cosmos=ChainKeyNamespace::COSMOS, dev=ChainKeyNamespace::DEV)]
    InvalidNamespaceForCosmos(ChainKeyNamespace),
    #[error("Namespace for cosmos chain must be {evm} or {dev}, got {0}", evm=ChainKeyNamespace::EVM, dev=ChainKeyNamespace::DEV)]
    InvalidNamespaceForEvm(ChainKeyNamespace),
    #[error("Namespace must be one of {cosmos}, {evm}, or {dev}, got {0}", cosmos=ChainKeyNamespace::COSMOS, evm=ChainKeyNamespace::EVM, dev=ChainKeyNamespace::DEV)]
    InvalidNamespace(ChainKeyNamespace),
    #[error("Chain ID mismatch: expected {expected}, got {actual}")]
    IdMismatch {
        expected: ChainKeyId,
        actual: ChainKeyId,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct CosmosChainConfig {
    pub chain_id: ChainKeyId,
    pub bech32_prefix: String,
    pub rpc_endpoint: Option<String>,
    pub grpc_endpoint: Option<String>,
    pub gas_price: f32,
    pub gas_denom: String,
    pub faucet_endpoint: Option<String>,
}

impl From<&CosmosChainConfig> for ChainKey {
    fn from(config: &CosmosChainConfig) -> Self {
        ChainKey {
            id: config.chain_id.clone(),
            namespace: ChainKeyNamespace::COSMOS.parse().unwrap(),
        }
    }
}

impl From<CosmosChainConfig> for ChainKey {
    fn from(config: CosmosChainConfig) -> Self {
        (&config).into()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct EvmChainConfig {
    pub chain_id: ChainKeyId,
    pub ws_endpoint: Option<String>,
    pub http_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    pub poll_interval_ms: Option<u64>,
    #[serde(default = "EvmChainConfig::default_event_channel_size")]
    /// Maximum number of buffered pubsub messages before we start dropping them; defaults to `DEFAULT_CHANNEL_SIZE`.
    pub event_channel_size: usize,
}

impl EvmChainConfig {
    /// Default buffer large enough to absorb short spikes (≈20 MiB if logs average 1 KiB) without overwhelming memory.
    pub const fn default_event_channel_size() -> usize {
        20_000
    }
}

impl From<&EvmChainConfig> for ChainKey {
    fn from(config: &EvmChainConfig) -> Self {
        ChainKey {
            id: config.chain_id.clone(),
            namespace: ChainKeyNamespace::EVM.parse().unwrap(),
        }
    }
}

impl From<EvmChainConfig> for ChainKey {
    fn from(config: EvmChainConfig) -> Self {
        (&config).into()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnyChainConfig {
    Cosmos(CosmosChainConfig),
    Evm(EvmChainConfig),
}

impl From<&AnyChainConfig> for ChainKey {
    fn from(config: &AnyChainConfig) -> Self {
        match config {
            AnyChainConfig::Cosmos(config) => config.into(),
            AnyChainConfig::Evm(config) => config.into(),
        }
    }
}

impl From<AnyChainConfig> for ChainKey {
    fn from(config: AnyChainConfig) -> Self {
        (&config).into()
    }
}

impl AnyChainConfig {
    pub fn chain_id(&self) -> &ChainKeyId {
        match self {
            AnyChainConfig::Cosmos(config) => &config.chain_id,
            AnyChainConfig::Evm(config) => &config.chain_id,
        }
    }
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
            chain_id: layer_climb::prelude::ChainId::new(config.chain_id),
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
            chain_id: config.chain_id.as_str().parse()?,
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
