use crate::worker::bindings::world::wavs::types::chain as component_chain;
use crate::worker::bindings::world::wavs::worker::input as component_input;
use crate::worker::bindings::world::wavs::worker::output as component_output;

impl From<wavs_types::WasmResponse> for component_output::WasmResponse {
    fn from(src: wavs_types::WasmResponse) -> Self {
        Self {
            payload: src.payload,
            ordering: src.ordering,
        }
    }
}

impl TryFrom<wavs_types::TriggerAction> for component_input::TriggerAction {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::TriggerAction) -> Result<Self, Self::Error> {
        Ok(Self {
            config: src.config.try_into()?,
            data: src.data.try_into()?,
        })
    }
}

impl TryFrom<wavs_types::TriggerConfig> for component_input::TriggerConfig {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::TriggerConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            service_id: src.service_id.to_string(),
            workflow_id: src.workflow_id.to_string(),
            trigger: src.trigger.try_into()?,
        })
    }
}

impl TryFrom<wavs_types::TriggerData> for component_input::TriggerData {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::TriggerData) -> Result<Self, Self::Error> {
        match src {
            wavs_types::TriggerData::EvmContractEvent {
                chain_name,
                contract_address,
                log_data,
                tx_hash,
                block_number,
                log_index,
                block_hash,
                block_timestamp,
                tx_index,
                removed,
            } => Ok(component_input::TriggerData::EvmContractEvent(
                component_input::TriggerDataEvmContractEvent {
                    chain_name: chain_name.to_string(),
                    log: component_input::EvmEventLog {
                        address: contract_address.into(),
                        data: component_chain::EvmEventLogData {
                            topics: log_data
                                .topics()
                                .iter()
                                .map(|topic| topic.to_vec())
                                .collect(),
                            data: log_data.data.to_vec(),
                        },
                        tx_hash: tx_hash.to_vec(),
                        block_number,
                        log_index,
                        block_hash: block_hash.map(|hash| hash.to_vec()),
                        block_timestamp,
                        tx_index,
                        removed,
                    },
                },
            )),
            wavs_types::TriggerData::CosmosContractEvent {
                contract_address,
                chain_name,
                event,
                event_index,
                block_height,
            } => Ok(component_input::TriggerData::CosmosContractEvent(
                component_input::TriggerDataCosmosContractEvent {
                    contract_address: contract_address.try_into()?,
                    chain_name: chain_name.to_string(),
                    event: component_input::CosmosEvent {
                        ty: event.ty,
                        attributes: event
                            .attributes
                            .into_iter()
                            .map(|attr| (attr.key, attr.value))
                            .collect(),
                    },
                    event_index,
                    block_height,
                },
            )),
            wavs_types::TriggerData::BlockInterval {
                chain_name,
                block_height,
            } => Ok(component_input::TriggerData::BlockInterval(
                component_input::TriggerDataBlockInterval {
                    chain_name: chain_name.to_string(),
                    block_height,
                },
            )),
            wavs_types::TriggerData::Cron { trigger_time } => Ok(
                component_input::TriggerData::Cron(component_input::TriggerDataCron {
                    trigger_time: trigger_time.into(),
                }),
            ),
            wavs_types::TriggerData::Raw(data) => Ok(component_input::TriggerData::Raw(data)),
        }
    }
}
