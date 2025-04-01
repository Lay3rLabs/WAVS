use crate::{bindings::world::wavs::worker::layer_types as component, EngineError};

impl TryFrom<wavs_types::TriggerAction> for component::TriggerAction {
    type Error = EngineError;

    fn try_from(src: wavs_types::TriggerAction) -> Result<Self, Self::Error> {
        Ok(Self {
            config: src.config.try_into()?,
            data: src.data.try_into()?,
        })
    }
}

impl TryFrom<wavs_types::TriggerConfig> for component::TriggerConfig {
    type Error = EngineError;

    fn try_from(src: wavs_types::TriggerConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            service_id: src.service_id.to_string(),
            workflow_id: src.workflow_id.to_string(),
            trigger_source: src.trigger.try_into()?,
        })
    }
}

impl TryFrom<wavs_types::Trigger> for component::TriggerSource {
    type Error = EngineError;

    fn try_from(src: wavs_types::Trigger) -> Result<Self, Self::Error> {
        Ok(match src {
            wavs_types::Trigger::CosmosContractEvent { address, chain_name, event_type } => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::CosmosContractEvent(
                    crate::bindings::world::wavs::worker::layer_types::TriggerSourceCosmosContractEvent {
                        address: address.try_into()?,
                        chain_name: chain_name.to_string(),
                        event_type,
                    }
                )
            },
            wavs_types::Trigger::EthContractEvent { address, chain_name, event_hash } => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::EthContractEvent(
                    crate::bindings::world::wavs::worker::layer_types::TriggerSourceEthContractEvent {
                        address: address.into(),
                        chain_name: chain_name.to_string(),
                        event_hash: event_hash.as_slice().to_vec(),
                    }
                )
            },
            wavs_types::Trigger::BlockInterval { chain_name, n_blocks } => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::BlockInterval(
                    crate::bindings::world::wavs::worker::layer_types::BlockIntervalSource {
                        chain_name: chain_name.to_string(),
                        n_blocks: n_blocks.into(),
                    }
                )
            },
            wavs_types::Trigger::Manual => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::Manual
            },
            wavs_types::Trigger::Cron { schedule, start_time, end_time } => {
                crate::bindings::world::wavs::worker::layer_types::TriggerSource::Cron(crate::bindings::world::wavs::worker::layer_types::TriggerSourceCron{
                    schedule: schedule.to_string(), start_time: start_time.map(Into::into), end_time: end_time.map(Into::into)
                })
            }
        })
    }
}

impl TryFrom<wavs_types::TriggerData> for component::TriggerData {
    type Error = EngineError;

    fn try_from(src: wavs_types::TriggerData) -> Result<Self, Self::Error> {
        match src {
            wavs_types::TriggerData::EthContractEvent {
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
            wavs_types::TriggerData::CosmosContractEvent { contract_address, chain_name, event, block_height } => {
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
            wavs_types::TriggerData::BlockInterval { chain_name, block_height } => {
                Ok(crate::bindings::world::wavs::worker::layer_types::TriggerData::BlockInterval(
                    crate::bindings::world::wavs::worker::layer_types::BlockIntervalData {
                        chain_name: chain_name.to_string(),
                        block_height,
                    }
                ))
            },
            wavs_types::TriggerData::Cron { execution_time, trigger_time } => Ok(crate::bindings::world::wavs::worker::layer_types::TriggerData::Cron(crate::bindings::world::wavs::worker::layer_types::TriggerDataCron { execution_time: execution_time.into(), trigger_time: trigger_time.into() })),
            wavs_types::TriggerData::Raw(data) => {
                Ok(crate::bindings::world::wavs::worker::layer_types::TriggerData::Raw(data))
            },
        }
    }
}

impl TryFrom<layer_climb::prelude::Address> for component::CosmosAddress {
    type Error = EngineError;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        let (bech32_addr, prefix_len) = match address {
            layer_climb::prelude::Address::Cosmos {
                bech32_addr,
                prefix_len,
            } => (bech32_addr, prefix_len),
            _ => {
                return Err(EngineError::TriggerData(anyhow::anyhow!(
                    "Not a cosmos address"
                )))
            }
        };

        Ok(Self {
            bech32_addr,
            prefix_len: prefix_len as u32,
        })
    }
}

impl TryFrom<layer_climb::prelude::Address> for component::EthAddress {
    type Error = EngineError;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match address {
            layer_climb::prelude::Address::Eth(addr) => Ok(Self {
                raw_bytes: addr.as_bytes().to_vec(),
            }),
            _ => Err(EngineError::TriggerData(anyhow::anyhow!(
                "Not an ethereum address"
            ))),
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

impl From<wavs_types::Timestamp> for component::Timestamp {
    fn from(src: wavs_types::Timestamp) -> Self {
        component::Timestamp {
            nanos: src.as_nanos(),
        }
    }
}

impl From<component::Timestamp> for wavs_types::Timestamp {
    fn from(src: component::Timestamp) -> Self {
        wavs_types::Timestamp::new(src.nanos)
    }
}
