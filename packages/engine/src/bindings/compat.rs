use crate::{bindings::world::wavs::types::core as component_core, bindings::world::wavs::types::service as component_service, EngineError};

impl From<wavs_types::WasmResponse> for component_core::WasmResponse {
    fn from(src: wavs_types::WasmResponse) -> Self {
        Self {
            payload: src.payload,
            ordering: src.ordering,
        }
    }
}

impl From<component_core::WasmResponse> for wavs_types::WasmResponse {
    fn from(src: component_core::WasmResponse) -> Self {
        Self {
            payload: src.payload,
            ordering: src.ordering,
        }
    }
}

impl TryFrom<wavs_types::TriggerAction> for component_core::TriggerAction {
    type Error = EngineError;

    fn try_from(src: wavs_types::TriggerAction) -> Result<Self, Self::Error> {
        Ok(Self {
            config: src.config.try_into()?,
            data: src.data.try_into()?,
        })
    }
}

impl TryFrom<wavs_types::TriggerConfig> for component_core::TriggerConfig {
    type Error = EngineError;

    fn try_from(src: wavs_types::TriggerConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            service_id: src.service_id.to_string(),
            workflow_id: src.workflow_id.to_string(),
            trigger: src.trigger.try_into()?,
        })
    }
}

impl TryFrom<wavs_types::Trigger> for component_core::Trigger {
    type Error = EngineError;

    fn try_from(src: wavs_types::Trigger) -> Result<Self, Self::Error> {
        Ok(match src {
            wavs_types::Trigger::CosmosContractEvent { address, chain_name, event_type } => {
                component_core::Trigger::CosmosContractEvent(
                    component_core::TriggerSourceCosmosContractEvent {
                        address: address.try_into()?,
                        chain_name: chain_name.to_string(),
                        event_type,
                    }
                )
            },
            wavs_types::Trigger::EvmContractEvent { address, chain_name, event_hash } => {
                component_core::Trigger::EvmContractEvent(
                    component_core::TriggerSourceEvmContractEvent {
                        address: address.into(),
                        chain_name: chain_name.to_string(),
                        event_hash: event_hash.as_slice().to_vec(),
                    }
                )
            },
            wavs_types::Trigger::BlockInterval { chain_name, n_blocks, start_block, end_block } => {
                component_core::Trigger::BlockInterval(
                    component_core::BlockIntervalSource {
                        chain_name: chain_name.to_string(),
                        n_blocks: n_blocks.into(),
                        start_block: start_block.map(Into::into),
                        end_block: end_block.map(Into::into),
                    }
                )
            },
            wavs_types::Trigger::Manual => {
                component_core::Trigger::Manual
            },
            wavs_types::Trigger::Cron { schedule, start_time, end_time } => {
                component_core::Trigger::Cron(component_core::TriggerSourceCron{
                    schedule: schedule.to_string(), start_time: start_time.map(Into::into), end_time: end_time.map(Into::into)
                })
            }
        })
    }
}

impl TryFrom<wavs_types::TriggerData> for component_core::TriggerData {
    type Error = EngineError;

    fn try_from(src: wavs_types::TriggerData) -> Result<Self, Self::Error> {
        match src {
            wavs_types::TriggerData::EvmContractEvent {
                contract_address,
                chain_name,
                log,
                block_height,
            } => {
                Ok(component_core::TriggerData::EvmContractEvent(
                    component_core::TriggerDataEvmContractEvent {
                        contract_address: component_core::EvmAddress {
                            raw_bytes: contract_address.to_vec()
                        },
                        chain_name: chain_name.to_string(),
                        log: component_core::EvmEventLogData {
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
                Ok(component_core::TriggerData::CosmosContractEvent(
                    component_core::TriggerDataCosmosContractEvent {
                        contract_address: contract_address.try_into()?,
                        chain_name: chain_name.to_string(),
                        event: component_core::CosmosEvent {
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
                Ok(component_core::TriggerData::BlockInterval(
                    component_core::BlockIntervalData {
                        chain_name: chain_name.to_string(),
                        block_height,
                    }
                ))
            },
            wavs_types::TriggerData::Cron { trigger_time } => Ok(component_core::TriggerData::Cron(component_core::TriggerDataCron {  trigger_time: trigger_time.into() })),
            wavs_types::TriggerData::Raw(data) => {
                Ok(component_core::TriggerData::Raw(data))
            },
        }
    }
}

impl TryFrom<layer_climb::prelude::Address> for component_core::CosmosAddress {
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

impl TryFrom<layer_climb::prelude::Address> for component_core::EvmAddress {
    type Error = EngineError;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match address {
            layer_climb::prelude::Address::Evm(addr) => Ok(Self {
                raw_bytes: addr.as_bytes().to_vec(),
            }),
            _ => Err(EngineError::TriggerData(anyhow::anyhow!(
                "Not an EVM address"
            ))),
        }
    }
}

impl From<alloy_primitives::Address> for component_core::EvmAddress {
    fn from(address: alloy_primitives::Address) -> Self {
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

impl From<utils::config::EvmChainConfig> for super::world::host::EvmChainConfig {
    fn from(config: utils::config::EvmChainConfig) -> Self {
        Self {
            chain_id: config.chain_id,
            ws_endpoint: config.ws_endpoint,
            http_endpoint: config.http_endpoint,
        }
    }
}

impl From<wavs_types::Timestamp> for component_core::Timestamp {
    fn from(src: wavs_types::Timestamp) -> Self {
        component_core::Timestamp {
            nanos: src.as_nanos(),
        }
    }
}

impl From<component_core::Timestamp> for wavs_types::Timestamp {
    fn from(src: component_core::Timestamp) -> Self {
        wavs_types::Timestamp::from_nanos(src.nanos)
    }
}
