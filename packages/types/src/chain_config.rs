use std::collections::BTreeMap;

use layer_climb_address::AddrKind;
use layer_climb_config::ChainConfig;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{ChainKey, ChainKeyError, ChainKeyId, ChainKeyNamespace};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChainConfigError {
    #[error("Expected EVM chain")]
    ExpectedEvmChain,
    #[error("Expected Cosmos chain")]
    ExpectedCosmosChain,
    #[error("Expected Solana chain")]
    ExpectedSolanaChain,
    #[error("Invalid chain: {0}")]
    InvalidChainKey(#[from] ChainKeyError),
    #[error("Chain already exists: {0}")]
    DuplicateChain(ChainKey),
    #[error("Namespace for cosmos chain must be {cosmos} or {dev}, got {0}", cosmos=ChainKeyNamespace::COSMOS, dev=ChainKeyNamespace::DEV)]
    InvalidNamespaceForCosmos(ChainKeyNamespace),
    #[error("Namespace for cosmos chain must be {evm} or {dev}, got {0}", evm=ChainKeyNamespace::EVM, dev=ChainKeyNamespace::DEV)]
    InvalidNamespaceForEvm(ChainKeyNamespace),
    #[error("Namespace for solana chain must be {solana} or {dev}, got {0}", solana=ChainKeyNamespace::SOLANA, dev=ChainKeyNamespace::DEV)]
    InvalidNamespaceForSolana(ChainKeyNamespace),
    #[error("Namespace must be one of {cosmos}, {evm}, {solana}, or {dev}, got {0}", cosmos=ChainKeyNamespace::COSMOS, evm=ChainKeyNamespace::EVM, solana=ChainKeyNamespace::SOLANA, dev=ChainKeyNamespace::DEV)]
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
    pub ws_endpoints: Vec<String>,
    pub http_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    pub ws_priority_endpoint_index: Option<usize>,
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

/// Solana chain configuration.
///
/// `commitment` is the default commitment level used by trigger subscriptions
/// when a trigger does not specify its own commitment. See
/// [`crate::SolanaCommitment`] for the variants and their semantics.
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct SolanaChainConfig {
    pub chain_id: ChainKeyId,
    pub http_endpoint: Option<String>,
    pub ws_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    #[serde(default)]
    pub commitment: crate::SolanaCommitment,
}

impl From<&SolanaChainConfig> for ChainKey {
    fn from(config: &SolanaChainConfig) -> Self {
        ChainKey {
            id: config.chain_id.clone(),
            namespace: ChainKeyNamespace::SOLANA.parse().unwrap(),
        }
    }
}

impl From<SolanaChainConfig> for ChainKey {
    fn from(config: SolanaChainConfig) -> Self {
        (&config).into()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnyChainConfig {
    Cosmos(CosmosChainConfig),
    Evm(EvmChainConfig),
    Solana(SolanaChainConfig),
}

impl From<&AnyChainConfig> for ChainKey {
    fn from(config: &AnyChainConfig) -> Self {
        match config {
            AnyChainConfig::Cosmos(config) => config.into(),
            AnyChainConfig::Evm(config) => config.into(),
            AnyChainConfig::Solana(config) => config.into(),
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
            AnyChainConfig::Solana(config) => &config.chain_id,
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

impl From<SolanaChainConfig> for AnyChainConfig {
    fn from(config: SolanaChainConfig) -> Self {
        AnyChainConfig::Solana(config)
    }
}

impl TryFrom<AnyChainConfig> for CosmosChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Cosmos(config) => Ok(config),
            AnyChainConfig::Evm(_) | AnyChainConfig::Solana(_) => {
                Err(ChainConfigError::ExpectedCosmosChain)
            }
        }
    }
}

impl TryFrom<AnyChainConfig> for EvmChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Evm(config) => Ok(config),
            AnyChainConfig::Cosmos(_) | AnyChainConfig::Solana(_) => {
                Err(ChainConfigError::ExpectedEvmChain)
            }
        }
    }
}

impl TryFrom<AnyChainConfig> for SolanaChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Solana(config) => Ok(config),
            AnyChainConfig::Cosmos(_) | AnyChainConfig::Evm(_) => {
                Err(ChainConfigError::ExpectedSolanaChain)
            }
        }
    }
}

impl From<CosmosChainConfig> for ChainConfig {
    fn from(config: CosmosChainConfig) -> Self {
        Self {
            chain_id: layer_climb_config::ChainId::new(config.chain_id),
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
            AnyChainConfig::Evm(_) | AnyChainConfig::Solana(_) => {
                Err(ChainConfigError::ExpectedCosmosChain)
            }
        }
    }

    pub fn to_evm_config(&self) -> Result<EvmChainConfig, ChainConfigError> {
        match self {
            AnyChainConfig::Evm(config) => Ok(config.clone()),
            AnyChainConfig::Cosmos(_) | AnyChainConfig::Solana(_) => {
                Err(ChainConfigError::ExpectedEvmChain)
            }
        }
    }

    pub fn to_solana_config(&self) -> Result<SolanaChainConfig, ChainConfigError> {
        match self {
            AnyChainConfig::Solana(config) => Ok(config.clone()),
            AnyChainConfig::Cosmos(_) | AnyChainConfig::Evm(_) => {
                Err(ChainConfigError::ExpectedSolanaChain)
            }
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

/// Chains are identified by `ChainKey`, which is a combination of a namespace and id
/// for now - we natively support 4 namespaces: cosmos, evm, solana, and dev
#[derive(Debug, Default, Deserialize, Serialize, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct ChainConfigs {
    /// Cosmos-style chains (including Layer-SDK)
    #[serde(default)]
    pub cosmos: BTreeMap<ChainKeyId, CosmosChainConfigBuilder>,
    /// EVM-style chains
    #[serde(default)]
    pub evm: BTreeMap<ChainKeyId, EvmChainConfigBuilder>,
    /// Solana / SVM chains
    #[serde(default)]
    pub solana: BTreeMap<ChainKeyId, SolanaChainConfigBuilder>,
    /// DEV-only chains
    /// The key here can be different than the chain_id inside the config
    #[serde(default)]
    pub dev: BTreeMap<ChainKeyId, AnyChainConfig>,
}

impl ChainConfigs {
    pub fn get_chain(&self, key: &ChainKey) -> Option<AnyChainConfig> {
        match key.namespace.as_str() {
            ChainKeyNamespace::COSMOS => self
                .cosmos
                .get(&key.id)
                .map(|c| AnyChainConfig::Cosmos(c.clone().build(key.id.clone()))),
            ChainKeyNamespace::EVM => self
                .evm
                .get(&key.id)
                .map(|c| AnyChainConfig::Evm(c.clone().build(key.id.clone()))),
            ChainKeyNamespace::SOLANA => self
                .solana
                .get(&key.id)
                .map(|c| AnyChainConfig::Solana(c.clone().build(key.id.clone()))),
            ChainKeyNamespace::DEV => self.dev.get(&key.id).cloned(),
            _ => None,
        }
    }

    pub fn cosmos_iter(&self) -> impl Iterator<Item = CosmosChainConfig> + '_ {
        self.cosmos
            .iter()
            .map(|(id, config)| config.clone().build(id.clone()))
    }

    pub fn evm_iter(&self) -> impl Iterator<Item = EvmChainConfig> + '_ {
        self.evm
            .iter()
            .map(|(id, config)| config.clone().build(id.clone()))
    }

    pub fn solana_iter(&self) -> impl Iterator<Item = SolanaChainConfig> + '_ {
        self.solana
            .iter()
            .map(|(id, config)| config.clone().build(id.clone()))
    }

    pub fn dev_iter(&self) -> impl Iterator<Item = AnyChainConfig> + '_ {
        self.dev.values().cloned()
    }

    pub fn all_chain_keys(&self) -> Result<Vec<ChainKey>, anyhow::Error> {
        let mut keys = Vec::new();

        for id in self.evm.keys() {
            keys.push(ChainKey {
                namespace: ChainKeyNamespace::EVM.parse()?,
                id: id.clone(),
            });
        }
        for id in self.cosmos.keys() {
            keys.push(ChainKey {
                namespace: ChainKeyNamespace::COSMOS.parse()?,
                id: id.clone(),
            });
        }
        for id in self.solana.keys() {
            keys.push(ChainKey {
                namespace: ChainKeyNamespace::SOLANA.parse()?,
                id: id.clone(),
            });
        }
        for id in self.dev.keys() {
            keys.push(ChainKey {
                namespace: ChainKeyNamespace::DEV.parse()?,
                id: id.clone(),
            });
        }

        Ok(keys)
    }

    pub fn chain_keys(&self, namespace: ChainKeyNamespace) -> Vec<ChainKey> {
        match namespace.as_str() {
            ChainKeyNamespace::COSMOS => self
                .cosmos
                .keys()
                .map(|id| ChainKey {
                    namespace: namespace.clone(),
                    id: id.clone(),
                })
                .collect(),
            ChainKeyNamespace::EVM => self
                .evm
                .keys()
                .map(|id| ChainKey {
                    namespace: namespace.clone(),
                    id: id.clone(),
                })
                .collect(),
            ChainKeyNamespace::SOLANA => self
                .solana
                .keys()
                .map(|id| ChainKey {
                    namespace: namespace.clone(),
                    id: id.clone(),
                })
                .collect(),
            ChainKeyNamespace::DEV => self
                .dev
                .keys()
                .map(|id| ChainKey {
                    namespace: namespace.clone(),
                    id: id.clone(),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn add_chain(
        &mut self,
        key: ChainKey,
        config: AnyChainConfig,
    ) -> Result<(), ChainConfigError> {
        if self.get_chain(&key).is_some() {
            return Err(ChainConfigError::DuplicateChain(key));
        }

        match key.namespace.as_str() {
            ChainKeyNamespace::DEV => {
                self.dev.insert(key.id, config);
            }
            ChainKeyNamespace::EVM => match config {
                AnyChainConfig::Evm(evm_config) => {
                    if evm_config.chain_id != key.id {
                        return Err(ChainConfigError::IdMismatch {
                            expected: key.id,
                            actual: evm_config.chain_id,
                        });
                    }
                    let evm_config = EvmChainConfigBuilder {
                        ws_endpoints: evm_config.ws_endpoints,
                        http_endpoint: evm_config.http_endpoint,
                        faucet_endpoint: evm_config.faucet_endpoint,
                        ws_priority_endpoint_index: evm_config.ws_priority_endpoint_index,
                    };
                    self.evm.insert(key.id, evm_config);
                }
                _ => return Err(ChainConfigError::InvalidNamespaceForEvm(key.namespace)),
            },
            ChainKeyNamespace::COSMOS => match config {
                AnyChainConfig::Cosmos(cosmos_config) => {
                    if cosmos_config.chain_id != key.id {
                        return Err(ChainConfigError::IdMismatch {
                            expected: key.id,
                            actual: cosmos_config.chain_id,
                        });
                    }
                    let cosmos_config = CosmosChainConfigBuilder {
                        bech32_prefix: cosmos_config.bech32_prefix,
                        rpc_endpoint: cosmos_config.rpc_endpoint,
                        grpc_endpoint: cosmos_config.grpc_endpoint,
                        gas_price: cosmos_config.gas_price,
                        gas_denom: cosmos_config.gas_denom,
                        faucet_endpoint: cosmos_config.faucet_endpoint,
                    };
                    self.cosmos.insert(key.id, cosmos_config);
                }
                _ => return Err(ChainConfigError::InvalidNamespaceForCosmos(key.namespace)),
            },
            ChainKeyNamespace::SOLANA => match config {
                AnyChainConfig::Solana(solana_config) => {
                    if solana_config.chain_id != key.id {
                        return Err(ChainConfigError::IdMismatch {
                            expected: key.id,
                            actual: solana_config.chain_id,
                        });
                    }
                    let solana_config = SolanaChainConfigBuilder {
                        http_endpoint: solana_config.http_endpoint,
                        ws_endpoint: solana_config.ws_endpoint,
                        faucet_endpoint: solana_config.faucet_endpoint,
                        commitment: solana_config.commitment,
                    };
                    self.solana.insert(key.id, solana_config);
                }
                _ => return Err(ChainConfigError::InvalidNamespaceForSolana(key.namespace)),
            },
            _ => return Err(ChainConfigError::InvalidNamespace(key.namespace)),
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct CosmosChainConfigBuilder {
    pub bech32_prefix: String,
    pub rpc_endpoint: Option<String>,
    pub grpc_endpoint: Option<String>,
    pub gas_price: f32,
    pub gas_denom: String,
    pub faucet_endpoint: Option<String>,
}

impl CosmosChainConfigBuilder {
    pub fn build(self, id: ChainKeyId) -> CosmosChainConfig {
        CosmosChainConfig {
            chain_id: id,
            bech32_prefix: self.bech32_prefix,
            rpc_endpoint: self.rpc_endpoint,
            grpc_endpoint: self.grpc_endpoint,
            gas_price: self.gas_price,
            gas_denom: self.gas_denom,
            faucet_endpoint: self.faucet_endpoint,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct EvmChainConfigBuilder {
    pub ws_endpoints: Vec<String>,
    pub http_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    pub ws_priority_endpoint_index: Option<usize>,
}

impl EvmChainConfigBuilder {
    pub fn build(self, id: ChainKeyId) -> EvmChainConfig {
        EvmChainConfig {
            chain_id: id,
            ws_endpoints: self.ws_endpoints,
            http_endpoint: self.http_endpoint,
            faucet_endpoint: self.faucet_endpoint,
            ws_priority_endpoint_index: self.ws_priority_endpoint_index,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct SolanaChainConfigBuilder {
    pub http_endpoint: Option<String>,
    pub ws_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    #[serde(default)]
    pub commitment: crate::SolanaCommitment,
}

impl SolanaChainConfigBuilder {
    pub fn build(self, id: ChainKeyId) -> SolanaChainConfig {
        SolanaChainConfig {
            chain_id: id,
            http_endpoint: self.http_endpoint,
            ws_endpoint: self.ws_endpoint,
            faucet_endpoint: self.faucet_endpoint,
            commitment: self.commitment,
        }
    }
}

#[cfg(test)]
mod solana_chain_config_tests {
    use super::*;
    use crate::SolanaCommitment;

    fn sample() -> SolanaChainConfig {
        SolanaChainConfig {
            chain_id: "mainnet".parse().unwrap(),
            http_endpoint: Some("https://api.mainnet-beta.solana.com".into()),
            ws_endpoint: Some("wss://api.mainnet-beta.solana.com".into()),
            faucet_endpoint: None,
            commitment: SolanaCommitment::Confirmed,
        }
    }

    #[test]
    fn chain_key_from_solana_config() {
        let config = sample();
        let key: ChainKey = (&config).into();
        assert_eq!(key.namespace.as_str(), ChainKeyNamespace::SOLANA);
        assert_eq!(key.id.as_str(), "mainnet");
    }

    #[test]
    fn any_chain_config_solana_round_trip() {
        let config = sample();
        let any: AnyChainConfig = config.clone().into();
        let extracted: SolanaChainConfig = any.try_into().unwrap();
        assert_eq!(extracted.chain_id, config.chain_id);
        assert_eq!(extracted.commitment, config.commitment);
    }

    #[test]
    fn try_from_solana_into_cosmos_fails() {
        let any: AnyChainConfig = sample().into();
        let res: Result<CosmosChainConfig, _> = any.try_into();
        assert_eq!(res.unwrap_err(), ChainConfigError::ExpectedCosmosChain);
    }

    #[test]
    fn try_from_solana_into_evm_fails() {
        let any: AnyChainConfig = sample().into();
        let res: Result<EvmChainConfig, _> = any.try_into();
        assert_eq!(res.unwrap_err(), ChainConfigError::ExpectedEvmChain);
    }

    #[test]
    fn add_chain_inserts_solana() {
        let mut configs = ChainConfigs::default();
        let config = sample();
        let key: ChainKey = (&config).into();
        configs
            .add_chain(key.clone(), config.clone().into())
            .unwrap();
        let got = configs.get_chain(&key).unwrap();
        assert!(matches!(got, AnyChainConfig::Solana(_)));
    }

    #[test]
    fn add_chain_rejects_mismatched_id() {
        let mut configs = ChainConfigs::default();
        let mut config = sample();
        config.chain_id = "mainnet".parse().unwrap();
        let key = ChainKey {
            namespace: ChainKeyNamespace::SOLANA.parse().unwrap(),
            id: "devnet".parse().unwrap(),
        };
        let err = configs.add_chain(key, config.into()).unwrap_err();
        assert!(matches!(err, ChainConfigError::IdMismatch { .. }));
    }
}
