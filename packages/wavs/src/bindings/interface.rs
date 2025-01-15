use layer_wasi::bindings::interface::*;
use layer_wasi::{
    generate_any_enum_impls, generate_contract_chain_configs_impls, generate_contract_struct_impls,
    generate_struct_impls,
};

// The wasmtime bindgen doesn't have quite the same overlap of re-exports as the raw wit bindgen
// so not only do we need to generate our From/Into conversions,
// we need to be a bit more explicit about which ones are available
generate_struct_impls!(
    [
        (CosmosAddr, super::worlds::cosmos_contract_event::CosmosAddr),
        (CosmosAddr, super::worlds::any_contract_event::CosmosAddr)
    ],
    bech32_addr,
    prefix_len
);

generate_struct_impls!(
    [
        (
            CosmosEvent,
            super::worlds::cosmos_contract_event::CosmosEvent
        ),
        (CosmosEvent, super::worlds::any_contract_event::CosmosEvent)
    ],
    ty,
    attributes
);

generate_contract_struct_impls!(
    CosmosChainConfig,
    [
        chain_id,
        rpc_endpoint,
        grpc_endpoint,
        grpc_web_endpoint,
        gas_price,
        gas_denom,
        bech32_prefix
    ]
);

generate_struct_impls!(
    [
        (EthAddr, super::worlds::eth_contract_event::EthAddr),
        (EthAddr, super::worlds::any_contract_event::EthAddr)
    ],
    raw_bytes
);

generate_struct_impls!(
    [
        (
            EthEventLogData,
            super::worlds::eth_contract_event::EthEventLogData
        ),
        (
            EthEventLogData,
            super::worlds::any_contract_event::EthEventLogData
        )
    ],
    topics,
    data
);

generate_contract_struct_impls!(EthChainConfig, [chain_id, ws_endpoint, http_endpoint]);

generate_any_enum_impls!(AnyAddr, super::worlds::any_contract_event::AnyAddr);
generate_any_enum_impls!(AnyEvent, super::worlds::any_contract_event::AnyEvent);

generate_contract_chain_configs_impls!(ChainConfigs);

macro_rules! local_generate_chain_configs_impls {
    ($Type:ty) => {
        impl From<utils::config::ChainConfigs> for $Type {
            fn from(src: utils::config::ChainConfigs) -> Self {
                Self {
                    eth: src
                        .eth
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                    cosmos: src
                        .cosmos
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                }
            }
        }

        impl From<$Type> for utils::config::ChainConfigs {
            fn from(src: $Type) -> Self {
                Self {
                    eth: src
                        .eth
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                    cosmos: src
                        .cosmos
                        .into_iter()
                        .map(|(key, config)| (key.clone(), config.into()))
                        .collect(),
                }
            }
        }
    };
}

macro_rules! local_generate_eth_chain_config_impls {
    ($Type:ty) => {
        impl From<utils::config::EthereumChainConfig> for $Type {
            fn from(src: utils::config::EthereumChainConfig) -> Self {
                Self {
                    chain_id: src.chain_id,
                    ws_endpoint: match src.ws_endpoint.is_empty() {
                        true => None,
                        false => Some(src.ws_endpoint),
                    },
                    http_endpoint: src.http_endpoint,
                }
            }
        }

        impl From<$Type> for utils::config::EthereumChainConfig {
            fn from(src: $Type) -> Self {
                Self {
                    chain_id: src.chain_id,
                    ws_endpoint: src.ws_endpoint.unwrap_or_default(),
                    http_endpoint: src.http_endpoint,
                    aggregator_endpoint: None,
                    faucet_endpoint: None,
                }
            }
        }
    };
}

macro_rules! local_generate_cosmos_chain_config_impls {
    ($Type:ty) => {
        impl From<utils::config::CosmosChainConfig> for $Type {
            fn from(src: utils::config::CosmosChainConfig) -> Self {
                Self {
                    chain_id: src.chain_id,
                    bech32_prefix: src.bech32_prefix,
                    rpc_endpoint: src.rpc_endpoint,
                    grpc_endpoint: src.grpc_endpoint,
                    grpc_web_endpoint: None,
                    gas_price: src.gas_price,
                    gas_denom: src.gas_denom,
                }
            }
        }

        impl From<$Type> for utils::config::CosmosChainConfig {
            fn from(src: $Type) -> Self {
                Self {
                    chain_id: src.chain_id,
                    bech32_prefix: src.bech32_prefix,
                    rpc_endpoint: src.rpc_endpoint,
                    grpc_endpoint: src.grpc_endpoint,
                    gas_price: src.gas_price,
                    gas_denom: src.gas_denom,
                    faucet_endpoint: None,
                }
            }
        }
    };
}

local_generate_chain_configs_impls!(super::worlds::cosmos_contract_event::ChainConfigs);
local_generate_chain_configs_impls!(super::worlds::eth_contract_event::ChainConfigs);
local_generate_chain_configs_impls!(super::worlds::any_contract_event::ChainConfigs);

local_generate_eth_chain_config_impls!(super::worlds::cosmos_contract_event::EthChainConfig);
local_generate_eth_chain_config_impls!(super::worlds::eth_contract_event::EthChainConfig);
local_generate_eth_chain_config_impls!(super::worlds::any_contract_event::EthChainConfig);

local_generate_cosmos_chain_config_impls!(super::worlds::cosmos_contract_event::CosmosChainConfig);
local_generate_cosmos_chain_config_impls!(super::worlds::eth_contract_event::CosmosChainConfig);
local_generate_cosmos_chain_config_impls!(super::worlds::any_contract_event::CosmosChainConfig);
