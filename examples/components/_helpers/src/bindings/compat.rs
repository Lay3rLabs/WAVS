pub use super::world::wavs::worker::layer_types::*;

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

impl From<alloy_primitives::LogData> for EvmEventLogData {
    fn from(log_data: alloy_primitives::LogData) -> Self {
        EvmEventLogData {
            topics: log_data
                .topics()
                .iter()
                .map(|topic| topic.to_vec())
                .collect(),
            data: log_data.data.to_vec(),
        }
    }
}

impl From<EvmEventLogData> for alloy_primitives::LogData {
    fn from(log_data: EvmEventLogData) -> Self {
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

impl TryFrom<layer_climb::prelude::Address> for CosmosAddress {
    type Error = anyhow::Error;

    fn try_from(addr: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match addr {
            layer_climb::prelude::Address::Cosmos {
                bech32_addr,
                prefix_len,
            } => Ok(CosmosAddress {
                bech32_addr,
                prefix_len: prefix_len as u32,
            }),
            _ => Err(anyhow::anyhow!("Cannot convert to CosmosAddr")),
        }
    }
}

impl From<CosmosAddress> for layer_climb::prelude::Address {
    fn from(addr: CosmosAddress) -> Self {
        layer_climb::prelude::Address::Cosmos {
            bech32_addr: addr.bech32_addr,
            prefix_len: addr.prefix_len as usize,
        }
    }
}

impl TryFrom<layer_climb::prelude::Address> for EvmAddress {
    type Error = anyhow::Error;

    fn try_from(addr: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match addr {
            layer_climb::prelude::Address::Evm(eth) => Ok(EvmAddress {
                raw_bytes: eth.as_bytes().to_vec(),
            }),
            _ => Err(anyhow::anyhow!("Cannot convert to EthAddr")),
        }
    }
}

impl From<EvmAddress> for layer_climb::prelude::Address {
    fn from(addr: EvmAddress) -> Self {
        alloy_primitives::Address::from(addr).into()
    }
}

impl From<alloy_primitives::Address> for EvmAddress {
    fn from(addr: alloy_primitives::Address) -> Self {
        EvmAddress {
            raw_bytes: addr.to_vec(),
        }
    }
}

impl From<EvmAddress> for alloy_primitives::Address {
    fn from(addr: EvmAddress) -> Self {
        alloy_primitives::Address::from_slice(&addr.raw_bytes)
    }
}

impl From<CosmosChainConfig> for layer_climb::prelude::ChainConfig {
    fn from(config: CosmosChainConfig) -> layer_climb::prelude::ChainConfig {
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

impl From<layer_climb::prelude::ChainConfig> for CosmosChainConfig {
    fn from(config: layer_climb::prelude::ChainConfig) -> CosmosChainConfig {
        CosmosChainConfig {
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
