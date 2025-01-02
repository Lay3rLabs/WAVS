use anyhow::Result;
use clap::Parser;
use figment::Figment;
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{error::ChainConfigError, eth_client::EthClientConfig};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChainConfigs {
    /// Cosmos-style chains (including Layer-SDK)
    pub cosmos: HashMap<String, CosmosChainConfig>,
    /// Ethereum-style chains
    pub eth: HashMap<String, EthereumChainConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AnyChainConfig {
    Cosmos(CosmosChainConfig),
    Eth(EthereumChainConfig),
}

impl TryFrom<AnyChainConfig> for CosmosChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> std::result::Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Cosmos(config) => Ok(config),
            AnyChainConfig::Eth(_) => Err(ChainConfigError::ExpectedCosmosChain),
        }
    }
}

impl TryFrom<AnyChainConfig> for ChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> std::result::Result<Self, Self::Error> {
        CosmosChainConfig::try_from(config).map(Into::into)
    }
}

impl TryFrom<AnyChainConfig> for EthereumChainConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> std::result::Result<Self, Self::Error> {
        match config {
            AnyChainConfig::Eth(config) => Ok(config),
            AnyChainConfig::Cosmos(_) => Err(ChainConfigError::ExpectedEthChain),
        }
    }
}

impl TryFrom<AnyChainConfig> for EthClientConfig {
    type Error = ChainConfigError;

    fn try_from(config: AnyChainConfig) -> std::result::Result<Self, Self::Error> {
        EthereumChainConfig::try_from(config).map(Into::into)
    }
}

impl ChainConfigs {
    pub fn get_chain(&self, chain_name: &str) -> Result<Option<AnyChainConfig>> {
        match (self.eth.get(chain_name), self.cosmos.get(chain_name)) {
            (Some(_), Some(_)) => {
                Err(ChainConfigError::DuplicateChainName(chain_name.to_string()).into())
            }
            (Some(eth), None) => Ok(Some(AnyChainConfig::Eth(eth.clone()))),
            (None, Some(cosmos)) => Ok(Some(AnyChainConfig::Cosmos(cosmos.clone()))),
            (None, None) => Ok(None),
        }
    }

    pub fn merge_overrides(self, chain_config_override: &OptionalWavsChainConfig) -> Result<Self> {
        // The optional overrides use a prefix to distinguish between layer and ethereum fields
        // since in the CLI they get flattened and would conflict without a prefix
        // in order to cleanly merge it with our final, real chain config
        // we need to strip that prefix so that the fields match
        #[derive(Clone, Debug, Serialize, Deserialize, Default)]
        struct EthConfigOverride {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub chain_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub http_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ws_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub aggregator_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub submission_mnemonic: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub faucet_endpoint: Option<String>,
        }

        let eth_config_override = EthConfigOverride {
            chain_id: chain_config_override.chain_id.clone(),
            http_endpoint: chain_config_override.http_endpoint.clone(),
            ws_endpoint: chain_config_override.ws_endpoint.clone(),
            aggregator_endpoint: chain_config_override.aggregator_endpoint.clone(),
            submission_mnemonic: chain_config_override.submission_mnemonic.clone(),
            faucet_endpoint: chain_config_override.faucet_endpoint.clone(),
        };

        // The optional overrides use a prefix to distinguish between layer and ethereum fields
        // since in the CLI they get flattened and would conflict without a prefix
        // in order to cleanly merge it with our final, real chain config
        // we need to strip that prefix so that the fields match
        #[derive(Clone, Debug, Serialize, Deserialize, Default)]
        struct CosmosConfigOverride {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub chain_id: Option<ChainId>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub bech32_prefix: Option<ChainId>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub rpc_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub grpc_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub gas_price: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub gas_denom: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub faucet_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub submission_mnemonic: Option<String>,
        }

        let cosmos_config_override = CosmosConfigOverride {
            chain_id: chain_config_override.cosmos_chain_id.clone(),
            bech32_prefix: chain_config_override.cosmos_bech32_prefix.clone(),
            grpc_endpoint: chain_config_override.cosmos_grpc_endpoint.clone(),
            gas_price: chain_config_override.cosmos_gas_price,
            gas_denom: chain_config_override.cosmos_gas_denom.clone(),
            faucet_endpoint: chain_config_override.cosmos_faucet_endpoint.clone(),
            submission_mnemonic: chain_config_override.cosmos_submission_mnemonic.clone(),
            rpc_endpoint: chain_config_override.cosmos_rpc_endpoint.clone(),
        };

        let mut eth = HashMap::with_capacity(self.eth.len());
        for (name, config) in self.eth.into_iter() {
            let config_merged = Figment::new()
                .merge(figment::providers::Serialized::defaults(config))
                .merge(figment::providers::Serialized::defaults(
                    &eth_config_override,
                ))
                .extract()?;

            eth.insert(name, config_merged);
        }

        let mut cosmos = HashMap::with_capacity(self.cosmos.len());

        for (name, config) in self.cosmos.into_iter() {
            let config_merged = Figment::new()
                .merge(figment::providers::Serialized::defaults(config))
                .merge(figment::providers::Serialized::defaults(
                    &cosmos_config_override,
                ))
                .extract()?;

            cosmos.insert(name, config_merged);
        }

        Ok(Self { cosmos, eth })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CosmosChainConfig {
    pub chain_id: String,
    pub bech32_prefix: String,
    pub rpc_endpoint: Option<String>,
    pub grpc_endpoint: Option<String>,
    pub gas_price: f32,
    pub gas_denom: String,
    pub faucet_endpoint: Option<String>,
    /// mnemonic for the submission client (usually leave this as None and override in env)
    pub submission_mnemonic: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EthereumChainConfig {
    pub chain_id: String,
    pub ws_endpoint: String,
    pub http_endpoint: String,
    pub aggregator_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    /// mnemonic for the submission client (usually leave this as None and override in env)
    pub submission_mnemonic: Option<String>,
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

impl From<EthereumChainConfig> for EthClientConfig {
    fn from(config: EthereumChainConfig) -> Self {
        Self {
            ws_endpoint: Some(config.ws_endpoint),
            http_endpoint: config.http_endpoint,
            mnemonic: config.submission_mnemonic,
            hd_index: None,
            transport: None,
        }
    }
}

// flattened and used in both direct config and cli/env args
// because it is flattened, we need to use prefixes to avoid conflicts
#[derive(Parser, Clone, Debug, Serialize, Deserialize, Default)]
pub struct OptionalWavsChainConfig {
    /// To override the chosen eth chain's chain_id
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    /// To override the chosen eth chain's ws_endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_endpoint: Option<String>,
    /// To override the chosen eth chain's rpc_endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_endpoint: Option<String>,
    /// To override the chosen aggregator endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregator_endpoint: Option<String>,
    /// To override the chosen eth chain's submission mnemonic
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_mnemonic: Option<String>,
    /// To override the chosen eth chain's submission mnemonic
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faucet_endpoint: Option<String>,

    /// To override the chosen cosmos chain's chain_id
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_chain_id: Option<ChainId>,
    /// To override the chosen cosmos chain's bech32_prefix
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_bech32_prefix: Option<ChainId>,
    /// To override the chosen cosmos chain's rpc_endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_rpc_endpoint: Option<String>,
    /// To override the chosen cosmos chain's grpc_endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_grpc_endpoint: Option<String>,
    /// To override the chosen cosmos chain's gas_price
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_gas_price: Option<f32>,
    /// To override the chosen cosmos chain's gas_denom
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_gas_denom: Option<String>,
    /// To override the chosen cosmos chain's faucet_endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_faucet_endpoint: Option<String>,
    /// To override the chosen cosmos chain's submission mnemonic
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_submission_mnemonic: Option<String>,
}

#[cfg(test)]
mod test {
    use super::{ChainConfigs, CosmosChainConfig, EthereumChainConfig};

    #[test]
    fn chain_name_lookup() {
        let chain_configs = mock_chain_configs();
        let chain: CosmosChainConfig = chain_configs
            .get_chain("cosmos")
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(chain.chain_id, "cosmos");

        let chain: EthereumChainConfig = chain_configs
            .get_chain("eth")
            .unwrap()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(chain.chain_id, "eth");
    }

    #[test]
    fn chain_name_lookup_fails_duplicate() {
        let mut chain_configs = mock_chain_configs();

        chain_configs.cosmos.insert(
            "eth".to_string(),
            CosmosChainConfig {
                chain_id: "eth".to_string(),
                bech32_prefix: "eth".to_string(),
                rpc_endpoint: Some("http://localhost:1317".to_string()),
                grpc_endpoint: Some("http://localhost:9090".to_string()),
                gas_price: 0.01,
                gas_denom: "uatom".to_string(),
                faucet_endpoint: Some("http://localhost:8000".to_string()),
                submission_mnemonic: Some("mnemonic".to_string()),
            },
        );

        assert!(chain_configs.get_chain("eth").is_err());
    }

    fn mock_chain_configs() -> ChainConfigs {
        ChainConfigs {
            cosmos: vec![
                (
                    "cosmos".to_string(),
                    CosmosChainConfig {
                        chain_id: "cosmos".to_string(),
                        bech32_prefix: "cosmos".to_string(),
                        rpc_endpoint: Some("http://localhost:1317".to_string()),
                        grpc_endpoint: Some("http://localhost:9090".to_string()),
                        gas_price: 0.01,
                        gas_denom: "uatom".to_string(),
                        faucet_endpoint: Some("http://localhost:8000".to_string()),
                        submission_mnemonic: Some("mnemonic".to_string()),
                    },
                ),
                (
                    "layer".to_string(),
                    CosmosChainConfig {
                        chain_id: "layer".to_string(),
                        bech32_prefix: "layer".to_string(),
                        rpc_endpoint: Some("http://localhost:1317".to_string()),
                        grpc_endpoint: Some("http://localhost:9090".to_string()),
                        gas_price: 0.01,
                        gas_denom: "uatom".to_string(),
                        faucet_endpoint: Some("http://localhost:8000".to_string()),
                        submission_mnemonic: Some("mnemonic".to_string()),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            eth: vec![
                (
                    "eth".to_string(),
                    EthereumChainConfig {
                        chain_id: "eth".to_string(),
                        ws_endpoint: "ws://localhost:8546".to_string(),
                        http_endpoint: "http://localhost:8545".to_string(),
                        aggregator_endpoint: Some("http://localhost:8000".to_string()),
                        faucet_endpoint: Some("http://localhost:8000".to_string()),
                        submission_mnemonic: Some("mnemonic".to_string()),
                    },
                ),
                (
                    "polygon".to_string(),
                    EthereumChainConfig {
                        chain_id: "polygon".to_string(),
                        ws_endpoint: "ws://localhost:8546".to_string(),
                        http_endpoint: "http://localhost:8545".to_string(),
                        aggregator_endpoint: Some("http://localhost:8000".to_string()),
                        faucet_endpoint: Some("http://localhost:8000".to_string()),
                        submission_mnemonic: Some("mnemonic".to_string()),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}
