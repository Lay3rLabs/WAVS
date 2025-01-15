// Exports the interface types and also impls all the From/Into helpers
// this is necessary because even if types are shared among modules
// their bindings.rs are different and seen as different types
mod helpers;
mod inner {
    wit_bindgen::generate!({
        world: "layer-sdk-world",
        path: "../../sdk/wit",
        async: true,
    });
}

pub use inner::lay3r::avs::layer_types::*;

use crate::{
    generate_contract_chain_configs_impls, generate_contract_enum_impls,
    generate_contract_struct_impls,
};

generate_contract_struct_impls!(CosmosAddr, [bech32_addr, prefix_len]);

generate_contract_struct_impls!(CosmosEvent, [ty, attributes]);

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

generate_contract_struct_impls!(EthAddr, [raw_bytes]);

generate_contract_struct_impls!(EthEventLogData, [topics, data]);

generate_contract_struct_impls!(EthChainConfig, [chain_id, ws_endpoint, http_endpoint]);

generate_contract_enum_impls!(AnyAddr);
generate_contract_enum_impls!(AnyEvent);

generate_contract_chain_configs_impls!(ChainConfigs);

impl From<CosmosEvent> for cosmwasm_std::Event {
    fn from(event: CosmosEvent) -> Self {
        cosmwasm_std::Event::new(event.ty).add_attributes(event.attributes)
    }
}

impl From<cosmwasm_std::Event> for CosmosEvent {
    fn from(event: cosmwasm_std::Event) -> Self {
        CosmosEvent {
            ty: event.ty,
            attributes: event
                .attributes
                .into_iter()
                .map(|attr| (attr.key, attr.value))
                .collect(),
        }
    }
}

impl From<cosmwasm_std::Event> for AnyEvent {
    fn from(event: cosmwasm_std::Event) -> Self {
        AnyEvent::Cosmos(event.into())
    }
}

impl From<alloy_primitives::LogData> for EthEventLogData {
    fn from(log_data: alloy_primitives::LogData) -> Self {
        EthEventLogData {
            topics: log_data
                .topics()
                .iter()
                .map(|topic| topic.to_vec())
                .collect(),
            data: log_data.data.to_vec(),
        }
    }
}

impl From<EthEventLogData> for alloy_primitives::LogData {
    fn from(log_data: EthEventLogData) -> Self {
        alloy_primitives::LogData::new(
            log_data
                .topics
                .into_iter()
                .map(|topic| alloy_primitives::FixedBytes::<32>::from_slice(&topic))
                .collect(),
            log_data.data.into(),
        )
        .unwrap()
    }
}

impl From<alloy_primitives::LogData> for AnyEvent {
    fn from(log_data: alloy_primitives::LogData) -> Self {
        AnyEvent::Eth(log_data.into())
    }
}

impl From<layer_climb_address::Address> for AnyAddr {
    fn from(addr: layer_climb_address::Address) -> Self {
        match addr {
            layer_climb_address::Address::Cosmos {
                bech32_addr,
                prefix_len,
            } => AnyAddr::Cosmos(CosmosAddr {
                bech32_addr,
                prefix_len: prefix_len as u32,
            }),
            layer_climb_address::Address::Eth(addr) => AnyAddr::Eth(EthAddr {
                raw_bytes: addr.as_bytes().to_vec(),
            }),
        }
    }
}

impl From<AnyAddr> for layer_climb_address::Address {
    fn from(addr: AnyAddr) -> Self {
        match addr {
            AnyAddr::Cosmos(CosmosAddr {
                bech32_addr,
                prefix_len,
            }) => layer_climb_address::Address::Cosmos {
                bech32_addr,
                prefix_len: prefix_len as usize,
            },
            AnyAddr::Eth(EthAddr { raw_bytes }) => layer_climb_address::Address::Eth(
                layer_climb_address::AddrEth::new_vec(raw_bytes).unwrap(),
            ),
        }
    }
}

impl TryFrom<AnyAddr> for CosmosAddr {
    type Error = anyhow::Error;

    fn try_from(addr: AnyAddr) -> Result<Self, Self::Error> {
        match addr {
            AnyAddr::Cosmos(cosmos) => Ok(cosmos),
            _ => Err(anyhow::anyhow!("Cannot convert to CosmosAddr")),
        }
    }
}

impl TryFrom<layer_climb_address::Address> for CosmosAddr {
    type Error = anyhow::Error;

    fn try_from(addr: layer_climb_address::Address) -> Result<Self, Self::Error> {
        match addr {
            layer_climb_address::Address::Cosmos {
                bech32_addr,
                prefix_len,
            } => Ok(CosmosAddr {
                bech32_addr,
                prefix_len: prefix_len as u32,
            }),
            _ => Err(anyhow::anyhow!("Cannot convert to CosmosAddr")),
        }
    }
}

impl TryFrom<AnyAddr> for EthAddr {
    type Error = anyhow::Error;

    fn try_from(addr: AnyAddr) -> Result<Self, Self::Error> {
        match addr {
            AnyAddr::Eth(eth) => Ok(eth),
            _ => Err(anyhow::anyhow!("Cannot convert to EthAddr")),
        }
    }
}

impl TryFrom<layer_climb_address::Address> for EthAddr {
    type Error = anyhow::Error;

    fn try_from(addr: layer_climb_address::Address) -> Result<Self, Self::Error> {
        match addr {
            layer_climb_address::Address::Eth(eth) => Ok(EthAddr {
                raw_bytes: eth.as_bytes().to_vec(),
            }),
            _ => Err(anyhow::anyhow!("Cannot convert to EthAddr")),
        }
    }
}

impl From<alloy_primitives::Address> for EthAddr {
    fn from(addr: alloy_primitives::Address) -> Self {
        EthAddr {
            raw_bytes: addr.to_vec(),
        }
    }
}

impl From<EthAddr> for alloy_primitives::Address {
    fn from(addr: EthAddr) -> Self {
        alloy_primitives::Address::from_slice(&addr.raw_bytes)
    }
}

impl From<CosmosChainConfig> for layer_climb_config::ChainConfig {
    fn from(config: CosmosChainConfig) -> layer_climb_config::ChainConfig {
        layer_climb_config::ChainConfig {
            chain_id: layer_climb_config::ChainId::new(config.chain_id),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: config.grpc_web_endpoint,
            gas_denom: config.gas_denom,
            gas_price: config.gas_price,
            address_kind: layer_climb_config::AddrKind::Cosmos {
                prefix: config.bech32_prefix,
            },
        }
    }
}
