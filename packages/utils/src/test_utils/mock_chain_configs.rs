use std::collections::BTreeMap;

use crate::config::{ChainConfigs, CosmosChainConfigBuilder, EvmChainConfigBuilder};

pub fn mock_chain_configs() -> ChainConfigs {
    ChainConfigs {
        evm: vec![(
            "31337".try_into().unwrap(),
            EvmChainConfigBuilder {
                ws_endpoint: Some("ws://localhost:8546".to_string()),
                http_endpoint: Some("http://localhost:8545".to_string()),
                faucet_endpoint: None,
                poll_interval_ms: None,
            },
        )]
        .into_iter()
        .collect(),
        cosmos: vec![(
            "cosmos".try_into().unwrap(),
            CosmosChainConfigBuilder {
                rpc_endpoint: Some("http://localhost:26657".to_string()),
                grpc_endpoint: Some("http://localhost:9090".to_string()),
                bech32_prefix: "cosmos".to_string(),
                gas_denom: "ustake".to_string(),
                gas_price: 0.025,
                faucet_endpoint: None,
            },
        )]
        .into_iter()
        .collect(),
        dev: BTreeMap::new(),
    }
}
