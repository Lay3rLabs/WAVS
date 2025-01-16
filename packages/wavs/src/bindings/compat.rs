use crate::apis::trigger as api;
use crate::bindings::world::lay3r::avs::layer_types as component;

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

impl TryFrom<api::Trigger> for component::TriggerSource {
    type Error = api::TriggerError;

    fn try_from(src: api::Trigger) -> Result<Self, Self::Error> {
        Ok(match src {
            api::Trigger::CosmosContractEvent { address, chain_name, event_type } => {
                crate::bindings::world::lay3r::avs::layer_types::TriggerSource::CosmosContractEvent(
                    crate::bindings::world::lay3r::avs::layer_types::TriggerSourceCosmosContractEvent {
                        address: address.try_into()?,
                        chain_name,
                        event_type,
                    }
                )
            },
            api::Trigger::EthContractEvent { address, chain_name, event_hash } => {
                crate::bindings::world::lay3r::avs::layer_types::TriggerSource::EthContractEvent(
                    crate::bindings::world::lay3r::avs::layer_types::TriggerSourceEthContractEvent {
                        address: address.try_into()?,
                        chain_name,
                        event_hash: event_hash.to_vec(),
                    }
                )
            },
            api::Trigger::Manual => {
                crate::bindings::world::lay3r::avs::layer_types::TriggerSource::Manual
            },
        })
    }
}

impl TryFrom<api::TriggerData> for component::TriggerData {
    type Error = api::TriggerError;

    fn try_from(src: api::TriggerData) -> Result<Self, Self::Error> {
        match src {
            api::TriggerData::EthContractEvent {
                contract_address,
                chain_name,
                log,
                block_height,
            } => {
                Ok(crate::bindings::world::lay3r::avs::layer_types::TriggerData::EthContractEvent(
                    crate::bindings::world::lay3r::avs::layer_types::TriggerDataEthContractEvent {
                        contract_address: crate::bindings::world::lay3r::avs::layer_types::EthAddress {
                            raw_bytes: contract_address.as_bytes()
                        },
                        chain_name,
                        log: crate::bindings::world::lay3r::avs::layer_types::EthEventLogData {
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
            api::TriggerData::CosmosContractEvent { contract_address, chain_name, event, block_height } => {
                Ok(crate::bindings::world::lay3r::avs::layer_types::TriggerData::CosmosContractEvent(
                    crate::bindings::world::lay3r::avs::layer_types::TriggerDataCosmosContractEvent {
                        contract_address: contract_address.try_into()?,
                        chain_name,
                        event: crate::bindings::world::lay3r::avs::layer_types::CosmosEvent {
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
            api::TriggerData::Raw(data) => {
                Ok(crate::bindings::world::lay3r::avs::layer_types::TriggerData::Raw(data))
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
            ws_endpoint: match config.ws_endpoint.is_empty() {
                true => None,
                false => Some(config.ws_endpoint),
            },
            http_endpoint: config.http_endpoint,
        }
    }
}
