use layer_climb::prelude::CosmosAddr;

use super::world::wavs::types::chain as component_chain;

impl From<component_chain::CosmosEvent> for cosmwasm_std::Event {
    fn from(event: component_chain::CosmosEvent) -> Self {
        cosmwasm_std::Event::new(event.ty).add_attributes(event.attributes)
    }
}

impl From<cosmwasm_std::Event> for component_chain::CosmosEvent {
    fn from(event: cosmwasm_std::Event) -> Self {
        component_chain::CosmosEvent {
            ty: event.ty,
            attributes: event
                .attributes
                .into_iter()
                .map(|attr| (attr.key, attr.value))
                .collect(),
        }
    }
}

impl From<alloy_primitives::LogData> for component_chain::EvmEventLogData {
    fn from(log_data: alloy_primitives::LogData) -> Self {
        component_chain::EvmEventLogData {
            topics: log_data
                .topics()
                .iter()
                .map(|topic| topic.to_vec())
                .collect(),
            data: log_data.data.to_vec(),
        }
    }
}

impl From<component_chain::EvmEventLogData> for alloy_primitives::LogData {
    fn from(log_data: component_chain::EvmEventLogData) -> Self {
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

impl From<CosmosAddr> for component_chain::CosmosAddress {
    fn from(addr: CosmosAddr) -> Self {
        component_chain::CosmosAddress {
            bech32_addr: addr.to_string(),
            prefix_len: addr.prefix().len() as u32,
        }
    }
}

impl From<component_chain::CosmosAddress> for CosmosAddr {
    fn from(addr: component_chain::CosmosAddress) -> Self {
        CosmosAddr::new_unchecked(addr.bech32_addr, addr.prefix_len as usize)
    }
}

impl From<component_chain::CosmosAddress> for layer_climb::prelude::Address {
    fn from(addr: component_chain::CosmosAddress) -> Self {
        CosmosAddr::new_unchecked(addr.bech32_addr, addr.prefix_len as usize).into()
    }
}

impl TryFrom<layer_climb::prelude::Address> for component_chain::EvmAddress {
    type Error = anyhow::Error;

    fn try_from(addr: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match addr {
            layer_climb::prelude::Address::Evm(eth) => Ok(component_chain::EvmAddress {
                raw_bytes: eth.as_bytes().to_vec(),
            }),
            _ => Err(anyhow::anyhow!("Cannot convert to EthAddr")),
        }
    }
}

impl From<component_chain::EvmAddress> for layer_climb::prelude::Address {
    fn from(addr: component_chain::EvmAddress) -> Self {
        alloy_primitives::Address::from(addr).into()
    }
}

impl From<alloy_primitives::Address> for component_chain::EvmAddress {
    fn from(addr: alloy_primitives::Address) -> Self {
        component_chain::EvmAddress {
            raw_bytes: addr.to_vec(),
        }
    }
}

impl From<component_chain::EvmAddress> for alloy_primitives::Address {
    fn from(addr: component_chain::EvmAddress) -> Self {
        alloy_primitives::Address::from_slice(&addr.raw_bytes)
    }
}

impl From<component_chain::CosmosChainConfig> for layer_climb::prelude::ChainConfig {
    fn from(config: component_chain::CosmosChainConfig) -> layer_climb::prelude::ChainConfig {
        layer_climb::prelude::ChainConfig {
            chain_id: layer_climb::prelude::ChainId::new(config.chain_id),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: config.grpc_web_endpoint,
            gas_denom: config.gas_denom,
            gas_price: config.gas_price,
            address_kind: layer_climb::prelude::AddrKind::Cosmos {
                prefix: config.bech32_prefix,
            },
        }
    }
}

impl From<layer_climb::prelude::ChainConfig> for component_chain::CosmosChainConfig {
    fn from(config: layer_climb::prelude::ChainConfig) -> component_chain::CosmosChainConfig {
        component_chain::CosmosChainConfig {
            chain_id: config.chain_id.as_str().to_string(),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: config.grpc_web_endpoint,
            gas_denom: config.gas_denom,
            gas_price: config.gas_price,
            bech32_prefix: match config.address_kind {
                layer_climb::prelude::AddrKind::Cosmos { prefix } => prefix,
                _ => "".to_string(),
            },
        }
    }
}
