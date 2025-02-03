use crate::apis::trigger as api;
use crate::bindings::world::wavs::worker::layer_types as component;

impl TryFrom<api::TriggerAction> for component::TriggerAction {
    type Error = api::TriggerError;

    fn try_from(src: api::TriggerAction) -> Result<Self, Self::Error> {
        Ok(Self {
            config: src.config.try_into()?,
            data: src.data.try_into()?,
        })
    }
}

impl TryFrom<api::TriggerConfig> for component::TriggerConfig {
    type Error = api::TriggerError;

    fn try_from(src: api::TriggerConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            service_id: src.service_id.to_string(),
            workflow_id: src.workflow_id.to_string(),
            trigger_source: src.trigger.try_into()?,
        })
    }
}

impl TryFrom<utils::types::Trigger> for component::TriggerSource {
    type Error = api::TriggerError;

    fn try_from(src: utils::types::Trigger) -> Result<Self, Self::Error> {
        Ok(match src {
            utils::types::Trigger::CosmosContractEvent { address, chain_name, event_type } => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::CosmosContractEvent(
                    crate::bindings::world::wavs::worker::layer_types::TriggerSourceCosmosContractEvent {
                        address: address.try_into()?,
                        chain_name: chain_name.to_string(),
                        event_type,
                    }
                )
            },
            utils::types::Trigger::EthContractEvent { address, chain_name, event_hash } => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::EthContractEvent(
                    crate::bindings::world::wavs::worker::layer_types::TriggerSourceEthContractEvent {
                        address: address.into(),
                        chain_name: chain_name.to_string(),
                        event_hash: event_hash.to_vec(),
                    }
                )
            },
            utils::types::Trigger::Manual => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::Manual
            },
        })
    }
}

impl TryFrom<utils::types::TriggerData> for component::TriggerData {
    type Error = api::TriggerError;

    fn try_from(src: utils::types::TriggerData) -> Result<Self, Self::Error> {
        match src {
            utils::types::TriggerData::EthContractEvent {
                contract_address,
                chain_name,
                log,
                block_height,
            } => {
                Ok(crate::bindings::world::wavs::worker::layer_types::TriggerData::EthContractEvent(
                    crate::bindings::world::wavs::worker::layer_types::TriggerDataEthContractEvent {
                        contract_address: crate::bindings::world::wavs::worker::layer_types::EthAddress {
                            raw_bytes: contract_address.to_vec()
                        },
                        chain_name: chain_name.to_string(),
                        log: crate::bindings::world::wavs::worker::layer_types::EthEventLogData {
                            topics: log
                                .topics()
                                .iter()
                                .map(|topic| topic.to_vec())
                                .collect(),
                            data: log.data.to_vec(),
                        },
                        block_height,
                    }
                ))
            },
            utils::types::TriggerData::CosmosContractEvent { contract_address, chain_name, event, block_height } => {
                Ok(crate::bindings::world::wavs::worker::layer_types::TriggerData::CosmosContractEvent(
                    crate::bindings::world::wavs::worker::layer_types::TriggerDataCosmosContractEvent {
                        contract_address: contract_address.try_into()?,
                        chain_name: chain_name.to_string(),
                        event: crate::bindings::world::wavs::worker::layer_types::CosmosEvent {
                            ty: event.ty,
                            attributes: event
                                .attributes
                                .into_iter()
                                .map(|attr| (attr.key, attr.value))
                                .collect(),
                        },
                        block_height,
                    }
                ))
            },
            utils::types::TriggerData::Raw(data) => {
                Ok(crate::bindings::world::wavs::worker::layer_types::TriggerData::Raw(data))
            },
        }
    }
}

impl TryFrom<layer_climb::prelude::Address> for component::CosmosAddress {
    type Error = api::TriggerError;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        let (bech32_addr, prefix_len) = match address {
            layer_climb::prelude::Address::Cosmos {
                bech32_addr,
                prefix_len,
            } => (bech32_addr, prefix_len),
            _ => {
                return Err(api::TriggerError::TriggerDataParse(
                    "Not a cosmos address".to_string(),
                ))
            }
        };

        Ok(Self {
            bech32_addr,
            prefix_len: prefix_len as u32,
        })
    }
}

impl TryFrom<layer_climb::prelude::Address> for component::EthAddress {
    type Error = api::TriggerError;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match address {
            layer_climb::prelude::Address::Eth(addr) => Ok(Self {
                raw_bytes: addr.as_bytes().to_vec(),
            }),
            _ => Err(api::TriggerError::TriggerDataParse(
                "Not an ethereum address".to_string(),
            )),
        }
    }
}

impl From<alloy::primitives::Address> for component::EthAddress {
    fn from(address: alloy::primitives::Address) -> Self {
        Self {
            raw_bytes: address.to_vec(),
        }
    }
}

impl From<utils::config::CosmosChainConfig> for super::world::host::CosmosChainConfig {
    fn from(config: utils::config::CosmosChainConfig) -> Self {
        Self {
            chain_id: config.chain_id.as_str().to_string(),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: None,
            gas_denom: config.gas_denom,
            gas_price: config.gas_price,
            bech32_prefix: config.bech32_prefix,
        }
    }
}

impl From<utils::config::EthereumChainConfig> for super::world::host::EthChainConfig {
    fn from(config: utils::config::EthereumChainConfig) -> Self {
        Self {
            chain_id: config.chain_id,
            ws_endpoint: config.ws_endpoint,
            http_endpoint: config.http_endpoint,
        }
    }
}
