use crate::config::{ChainConfigs, CosmosChainConfig, EvmChainConfig};

pub fn mock_chain_configs() -> ChainConfigs {
    ChainConfigs {
        evm: vec![(
            "evm".try_into().unwrap(),
            EvmChainConfig {
                chain_id: 31337.to_string(),
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
            CosmosChainConfig {
                chain_id: "cosmos".to_string(),
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
        // TODO:
        svm: Default::default(),
    }
}
