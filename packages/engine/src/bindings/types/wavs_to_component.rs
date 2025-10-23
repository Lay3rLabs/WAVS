use wavs_wasi_utils::impl_u128_conversions;

use crate::bindings::{
    aggregator::world::wavs::{
        aggregator::aggregator::{self as aggregator_types, U128},
        types::{
            chain as aggregator_chain, core as aggregator_core, events as aggregator_events,
            service as aggregator_service,
        },
    },
    operator::world::wavs::{
        operator::{input as component_input, output as component_output},
        types::{
            chain as component_chain, core as component_core, events as component_events,
            service as component_service,
        },
    },
};

impl_u128_conversions!(U128);

impl TryFrom<wavs_types::Trigger> for component_service::Trigger {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::Trigger) -> Result<Self, Self::Error> {
        Ok(match src {
            wavs_types::Trigger::CosmosContractEvent {
                address,
                chain,
                event_type,
            } => component_service::Trigger::CosmosContractEvent(
                component_service::TriggerCosmosContractEvent {
                    address: address.into(),
                    chain: chain.to_string(),
                    event_type,
                },
            ),
            wavs_types::Trigger::EvmContractEvent {
                address,
                chain,
                event_hash,
            } => component_service::Trigger::EvmContractEvent(
                component_service::TriggerEvmContractEvent {
                    address: address.into(),
                    chain: chain.to_string(),
                    event_hash: event_hash.as_slice().to_vec(),
                },
            ),
            wavs_types::Trigger::BlockInterval {
                chain,
                n_blocks,
                start_block,
                end_block,
            } => {
                component_service::Trigger::BlockInterval(component_service::TriggerBlockInterval {
                    chain: chain.to_string(),
                    n_blocks: n_blocks.into(),
                    start_block: start_block.map(Into::into),
                    end_block: end_block.map(Into::into),
                })
            }
            wavs_types::Trigger::Manual => component_service::Trigger::Manual,
            wavs_types::Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => component_service::Trigger::Cron(component_service::TriggerCron {
                schedule: schedule.to_string(),
                start_time: start_time.map(Into::into),
                end_time: end_time.map(Into::into),
            }),
        })
    }
}

impl TryFrom<layer_climb::prelude::Address> for component_chain::CosmosAddress {
    type Error = anyhow::Error;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match address {
            layer_climb::prelude::Address::Cosmos(addr) => Ok(Self {
                bech32_addr: addr.to_string(),
                prefix_len: addr.prefix().len() as u32,
            }),
            _ => Err(anyhow::anyhow!("Not a cosmos address")),
        }
    }
}

impl TryFrom<layer_climb::prelude::Address> for component_chain::EvmAddress {
    type Error = anyhow::Error;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match address {
            layer_climb::prelude::Address::Evm(addr) => Ok(Self {
                raw_bytes: addr.as_bytes().to_vec(),
            }),
            _ => Err(anyhow::anyhow!("Not an EVM address")),
        }
    }
}

impl From<alloy_primitives::Address> for component_chain::EvmAddress {
    fn from(address: alloy_primitives::Address) -> Self {
        Self {
            raw_bytes: address.to_vec(),
        }
    }
}

impl From<layer_climb::prelude::CosmosAddr> for component_chain::CosmosAddress {
    fn from(address: layer_climb::prelude::CosmosAddr) -> Self {
        component_chain::CosmosAddress {
            bech32_addr: address.to_string(),
            prefix_len: address.prefix().len() as u32,
        }
    }
}

impl From<utils::config::CosmosChainConfig>
    for crate::bindings::operator::world::host::CosmosChainConfig
{
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

impl From<utils::config::EvmChainConfig>
    for crate::bindings::operator::world::host::EvmChainConfig
{
    fn from(config: utils::config::EvmChainConfig) -> Self {
        Self {
            chain_id: config.chain_id.to_string(),
            ws_endpoints: config.ws_endpoints,
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

impl TryFrom<wavs_types::Service> for component_service::Service {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::Service) -> Result<Self, Self::Error> {
        Ok(Self {
            name: src.name,
            workflows: src
                .workflows
                .into_iter()
                .map(|(workflow_id, workflow)| {
                    workflow
                        .try_into()
                        .map(|workflow| (workflow_id.to_string(), workflow))
                })
                .collect::<anyhow::Result<Vec<(String, component_service::Workflow)>>>()?,
            status: src.status.into(),
            manager: src.manager.into(),
        })
    }
}

impl TryFrom<wavs_types::Workflow> for component_service::Workflow {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::Workflow) -> Result<Self, Self::Error> {
        Ok(Self {
            trigger: src.trigger.try_into()?,
            component: src.component.into(),
            submit: src.submit.into(),
        })
    }
}

impl From<wavs_types::Component> for component_service::Component {
    fn from(src: wavs_types::Component) -> Self {
        Self {
            source: src.source.into(),
            permissions: src.permissions.into(),
            fuel_limit: src.fuel_limit,
            time_limit_seconds: src.time_limit_seconds,
            config: src.config.into_iter().collect(),
            env_keys: src.env_keys.into_iter().collect(),
        }
    }
}

impl From<wavs_types::ComponentSource> for component_service::ComponentSource {
    fn from(src: wavs_types::ComponentSource) -> Self {
        match src {
            wavs_types::ComponentSource::Digest(digest) => {
                component_service::ComponentSource::Digest(digest.to_string())
            }
            wavs_types::ComponentSource::Download { uri, digest } => {
                component_service::ComponentSource::Download(
                    component_service::ComponentSourceDownload {
                        uri: uri.to_string(),
                        digest: digest.to_string(),
                    },
                )
            }
            wavs_types::ComponentSource::Registry { registry } => {
                component_service::ComponentSource::Registry(registry.into())
            }
        }
    }
}

impl From<wavs_types::Registry> for component_service::Registry {
    fn from(src: wavs_types::Registry) -> Self {
        Self {
            digest: src.digest.to_string(),
            domain: src.domain,
            version: src.version.map(|v| v.to_string()),
            pkg: src.package.to_string(),
        }
    }
}

impl From<wavs_types::Permissions> for component_service::Permissions {
    fn from(src: wavs_types::Permissions) -> Self {
        Self {
            allowed_http_hosts: src.allowed_http_hosts.into(),
            file_system: src.file_system,
        }
    }
}

impl From<wavs_types::Submit> for component_service::Submit {
    fn from(src: wavs_types::Submit) -> Self {
        match src {
            wavs_types::Submit::None => component_service::Submit::None,
            wavs_types::Submit::Aggregator {
                url,
                component,
                signature_kind,
            } => component_service::Submit::Aggregator(component_service::AggregatorSubmit {
                url,
                component: (*component).into(),
                signature_kind: signature_kind.into(),
            }),
        }
    }
}

impl From<wavs_types::SignatureKind> for component_service::SignatureKind {
    fn from(src: wavs_types::SignatureKind) -> Self {
        Self {
            algorithm: src.algorithm.into(),
            prefix: src.prefix.map(Into::into),
        }
    }
}

impl From<wavs_types::SignatureAlgorithm> for component_service::SignatureAlgorithm {
    fn from(src: wavs_types::SignatureAlgorithm) -> Self {
        match src {
            wavs_types::SignatureAlgorithm::Secp256k1 => {
                component_service::SignatureAlgorithm::Secp256k1
            }
        }
    }
}

impl From<wavs_types::SignaturePrefix> for component_service::SignaturePrefix {
    fn from(src: wavs_types::SignaturePrefix) -> Self {
        match src {
            wavs_types::SignaturePrefix::Eip191 => component_service::SignaturePrefix::Eip191,
        }
    }
}

impl From<wavs_types::AllowedHostPermission> for component_service::AllowedHostPermission {
    fn from(src: wavs_types::AllowedHostPermission) -> Self {
        match src {
            wavs_types::AllowedHostPermission::All => component_service::AllowedHostPermission::All,
            wavs_types::AllowedHostPermission::None => {
                component_service::AllowedHostPermission::None
            }
            wavs_types::AllowedHostPermission::Only(hosts) => {
                component_service::AllowedHostPermission::Only(hosts)
            }
        }
    }
}

impl From<wavs_types::ServiceStatus> for component_service::ServiceStatus {
    fn from(src: wavs_types::ServiceStatus) -> Self {
        match src {
            wavs_types::ServiceStatus::Active => component_service::ServiceStatus::Active,
            wavs_types::ServiceStatus::Paused => component_service::ServiceStatus::Paused,
        }
    }
}

impl From<wavs_types::ServiceManager> for component_service::ServiceManager {
    fn from(src: wavs_types::ServiceManager) -> Self {
        match src {
            wavs_types::ServiceManager::Evm { chain, address } => {
                component_service::ServiceManager::Evm(component_service::EvmManager {
                    chain: chain.to_string(),
                    address: address.into(),
                })
            }
            wavs_types::ServiceManager::Cosmos { chain, address } => {
                component_service::ServiceManager::Cosmos(component_service::CosmosManager {
                    chain: chain.to_string(),
                    address: address.into(),
                })
            }
        }
    }
}

impl From<wavs_types::Aggregator> for component_service::Aggregator {
    fn from(src: wavs_types::Aggregator) -> Self {
        match src {
            wavs_types::Aggregator::Evm(evm) => component_service::Aggregator::Evm(evm.into()),
        }
    }
}

impl From<wavs_types::EvmContractSubmission> for component_service::EvmContractSubmission {
    fn from(src: wavs_types::EvmContractSubmission) -> Self {
        Self {
            chain: src.chain.to_string(),
            address: src.address.into(),
            max_gas: src.max_gas,
        }
    }
}

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
                chain,
                contract_address,
                log_data,
                tx_hash,
                block_number,
                log_index,
                block_hash,
                block_timestamp,
                tx_index,
            } => Ok(component_input::TriggerData::EvmContractEvent(
                component_events::TriggerDataEvmContractEvent {
                    chain: chain.to_string(),
                    log: component_events::EvmEventLog {
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
                        block_hash: block_hash.to_vec(),
                        block_timestamp,
                        tx_index,
                    },
                },
            )),
            wavs_types::TriggerData::CosmosContractEvent {
                contract_address,
                chain,
                event,
                event_index,
                block_height,
            } => Ok(component_input::TriggerData::CosmosContractEvent(
                component_events::TriggerDataCosmosContractEvent {
                    contract_address: contract_address.into(),
                    chain: chain.to_string(),
                    event: component_events::CosmosEvent {
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
                chain,
                block_height,
            } => Ok(component_input::TriggerData::BlockInterval(
                component_events::TriggerDataBlockInterval {
                    chain: chain.to_string(),
                    block_height,
                },
            )),
            wavs_types::TriggerData::Cron { trigger_time } => Ok(
                component_input::TriggerData::Cron(component_events::TriggerDataCron {
                    trigger_time: trigger_time.into(),
                }),
            ),
            wavs_types::TriggerData::Raw(data) => Ok(component_input::TriggerData::Raw(data)),
        }
    }
}

// aggregator

impl TryFrom<wavs_types::Packet> for aggregator_types::Packet {
    type Error = anyhow::Error;

    fn try_from(packet: wavs_types::Packet) -> Result<Self, Self::Error> {
        Ok(aggregator_types::Packet {
            service: packet.service.try_into()?,
            workflow_id: packet.workflow_id.to_string(),
            envelope: packet.envelope.into(),
            signature: packet.signature.into(),
            trigger_data: packet.trigger_data.try_into()?,
        })
    }
}

impl TryFrom<wavs_types::Service> for aggregator_service::Service {
    type Error = anyhow::Error;

    fn try_from(service: wavs_types::Service) -> Result<Self, Self::Error> {
        Ok(aggregator_service::Service {
            name: service.name,
            workflows: service
                .workflows
                .into_iter()
                .map(|(id, workflow)| (id.to_string(), workflow.try_into().unwrap()))
                .collect(),
            status: service.status.into(),
            manager: service.manager.into(),
        })
    }
}

impl TryFrom<wavs_types::Workflow> for aggregator_service::Workflow {
    type Error = anyhow::Error;

    fn try_from(workflow: wavs_types::Workflow) -> Result<Self, Self::Error> {
        Ok(aggregator_service::Workflow {
            trigger: workflow.trigger.try_into()?,
            component: workflow.component.into(),
            submit: workflow.submit.into(),
        })
    }
}

impl From<wavs_types::ServiceStatus> for aggregator_service::ServiceStatus {
    fn from(status: wavs_types::ServiceStatus) -> Self {
        match status {
            wavs_types::ServiceStatus::Active => aggregator_service::ServiceStatus::Active,
            wavs_types::ServiceStatus::Paused => aggregator_service::ServiceStatus::Paused,
        }
    }
}

impl From<wavs_types::ServiceManager> for aggregator_service::ServiceManager {
    fn from(manager: wavs_types::ServiceManager) -> Self {
        match manager {
            wavs_types::ServiceManager::Evm { chain, address } => {
                aggregator_service::ServiceManager::Evm(aggregator_service::EvmManager {
                    chain: chain.to_string(),
                    address: aggregator_chain::EvmAddress {
                        raw_bytes: address.to_vec(),
                    },
                })
            }
            wavs_types::ServiceManager::Cosmos { chain, address } => {
                aggregator_service::ServiceManager::Cosmos(aggregator_service::CosmosManager {
                    chain: chain.to_string(),
                    address: aggregator_chain::CosmosAddress {
                        bech32_addr: address.to_string(),
                        prefix_len: address.prefix().len() as u32,
                    },
                })
            }
        }
    }
}

impl From<alloy_primitives::Address> for aggregator_chain::EvmAddress {
    fn from(address: alloy_primitives::Address) -> Self {
        aggregator_chain::EvmAddress {
            raw_bytes: address.to_vec(),
        }
    }
}

impl From<layer_climb::prelude::CosmosAddr> for aggregator_chain::CosmosAddress {
    fn from(address: layer_climb::prelude::CosmosAddr) -> Self {
        aggregator_chain::CosmosAddress {
            bech32_addr: address.to_string(),
            prefix_len: address.prefix().len() as u32,
        }
    }
}

impl From<wavs_types::Envelope> for aggregator_types::Envelope {
    fn from(envelope: wavs_types::Envelope) -> Self {
        aggregator_types::Envelope {
            event_id: envelope.eventId.to_vec(),
            ordering: envelope.ordering.to_vec(),
            payload: envelope.payload.to_vec(),
        }
    }
}

impl From<wavs_types::EnvelopeSignature> for aggregator_types::EnvelopeSignature {
    fn from(signature: wavs_types::EnvelopeSignature) -> Self {
        let wavs_types::EnvelopeSignature { data, kind } = signature;
        aggregator_types::EnvelopeSignature {
            data,
            kind: kind.into(),
        }
    }
}

impl TryFrom<wavs_types::TriggerData> for aggregator_types::TriggerData {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::TriggerData) -> Result<Self, Self::Error> {
        match src {
            wavs_types::TriggerData::EvmContractEvent {
                chain,
                contract_address,
                log_data,
                tx_hash,
                block_number,
                log_index,
                block_hash,
                block_timestamp,
                tx_index,
            } => Ok(aggregator_types::TriggerData::EvmContractEvent(
                aggregator_events::TriggerDataEvmContractEvent {
                    chain: chain.to_string(),
                    log: aggregator_events::EvmEventLog {
                        address: contract_address.into(),
                        data: aggregator_chain::EvmEventLogData {
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
                        block_hash: block_hash.to_vec(),
                        block_timestamp,
                        tx_index,
                    },
                },
            )),
            wavs_types::TriggerData::CosmosContractEvent {
                contract_address,
                chain,
                event,
                event_index,
                block_height,
            } => Ok(aggregator_types::TriggerData::CosmosContractEvent(
                aggregator_events::TriggerDataCosmosContractEvent {
                    contract_address: contract_address.into(),
                    chain: chain.to_string(),
                    event: aggregator_events::CosmosEvent {
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
                chain,
                block_height,
            } => Ok(aggregator_types::TriggerData::BlockInterval(
                aggregator_events::TriggerDataBlockInterval {
                    chain: chain.to_string(),
                    block_height,
                },
            )),
            wavs_types::TriggerData::Cron { trigger_time } => Ok(
                aggregator_types::TriggerData::Cron(aggregator_events::TriggerDataCron {
                    trigger_time: trigger_time.into(),
                }),
            ),
            wavs_types::TriggerData::Raw(data) => Ok(aggregator_types::TriggerData::Raw(data)),
        }
    }
}

impl From<wavs_types::Component> for aggregator_service::Component {
    fn from(component: wavs_types::Component) -> Self {
        aggregator_service::Component {
            source: component.source.into(),
            permissions: component.permissions.into(),
            fuel_limit: component.fuel_limit,
            time_limit_seconds: component.time_limit_seconds,
            config: component.config.into_iter().collect(),
            env_keys: component.env_keys.into_iter().collect(),
        }
    }
}

impl From<wavs_types::ComponentSource> for aggregator_service::ComponentSource {
    fn from(source: wavs_types::ComponentSource) -> Self {
        match source {
            wavs_types::ComponentSource::Digest(digest) => {
                aggregator_service::ComponentSource::Digest(digest.to_string())
            }
            wavs_types::ComponentSource::Download { uri, digest } => {
                aggregator_service::ComponentSource::Download(
                    aggregator_service::ComponentSourceDownload {
                        uri: uri.to_string(),
                        digest: digest.to_string(),
                    },
                )
            }
            wavs_types::ComponentSource::Registry { registry } => {
                aggregator_service::ComponentSource::Registry(registry.into())
            }
        }
    }
}

impl From<wavs_types::Registry> for aggregator_service::Registry {
    fn from(registry: wavs_types::Registry) -> Self {
        aggregator_service::Registry {
            digest: registry.digest.to_string(),
            domain: registry.domain,
            version: registry.version.map(|v| v.to_string()),
            pkg: registry.package.to_string(),
        }
    }
}

impl From<wavs_types::Permissions> for aggregator_service::Permissions {
    fn from(permissions: wavs_types::Permissions) -> Self {
        aggregator_service::Permissions {
            allowed_http_hosts: permissions.allowed_http_hosts.into(),
            file_system: permissions.file_system,
        }
    }
}

impl From<wavs_types::AllowedHostPermission> for aggregator_service::AllowedHostPermission {
    fn from(permission: wavs_types::AllowedHostPermission) -> Self {
        match permission {
            wavs_types::AllowedHostPermission::All => {
                aggregator_service::AllowedHostPermission::All
            }
            wavs_types::AllowedHostPermission::None => {
                aggregator_service::AllowedHostPermission::None
            }
            wavs_types::AllowedHostPermission::Only(hosts) => {
                aggregator_service::AllowedHostPermission::Only(hosts)
            }
        }
    }
}

impl From<wavs_types::Submit> for aggregator_service::Submit {
    fn from(submit: wavs_types::Submit) -> Self {
        match submit {
            wavs_types::Submit::None => aggregator_service::Submit::None,
            wavs_types::Submit::Aggregator {
                url,
                component,
                signature_kind,
            } => aggregator_service::Submit::Aggregator(aggregator_service::AggregatorSubmit {
                url,
                component: (*component).into(),
                signature_kind: signature_kind.into(),
            }),
        }
    }
}

impl From<wavs_types::SignatureKind> for aggregator_service::SignatureKind {
    fn from(src: wavs_types::SignatureKind) -> Self {
        Self {
            algorithm: src.algorithm.into(),
            prefix: src.prefix.map(Into::into),
        }
    }
}

impl From<wavs_types::SignatureAlgorithm> for aggregator_service::SignatureAlgorithm {
    fn from(src: wavs_types::SignatureAlgorithm) -> Self {
        match src {
            wavs_types::SignatureAlgorithm::Secp256k1 => {
                aggregator_service::SignatureAlgorithm::Secp256k1
            }
        }
    }
}

impl From<wavs_types::SignaturePrefix> for aggregator_service::SignaturePrefix {
    fn from(src: wavs_types::SignaturePrefix) -> Self {
        match src {
            wavs_types::SignaturePrefix::Eip191 => aggregator_service::SignaturePrefix::Eip191,
        }
    }
}

impl From<wavs_types::EvmContractSubmission> for aggregator_service::EvmContractSubmission {
    fn from(submission: wavs_types::EvmContractSubmission) -> Self {
        aggregator_service::EvmContractSubmission {
            chain: submission.chain.to_string(),
            address: submission.address.into(),
            max_gas: submission.max_gas,
        }
    }
}

impl TryFrom<wavs_types::Trigger> for aggregator_service::Trigger {
    type Error = anyhow::Error;

    fn try_from(trigger: wavs_types::Trigger) -> Result<Self, Self::Error> {
        Ok(match trigger {
            wavs_types::Trigger::Manual => aggregator_service::Trigger::Manual,
            wavs_types::Trigger::EvmContractEvent {
                address,
                chain,
                event_hash,
            } => aggregator_service::Trigger::EvmContractEvent(
                aggregator_service::TriggerEvmContractEvent {
                    address: address.into(),
                    chain: chain.to_string(),
                    event_hash: event_hash.as_slice().to_vec(),
                },
            ),
            wavs_types::Trigger::CosmosContractEvent {
                address,
                chain,
                event_type,
            } => aggregator_service::Trigger::CosmosContractEvent(
                aggregator_service::TriggerCosmosContractEvent {
                    address: address.into(),
                    chain: chain.to_string(),
                    event_type,
                },
            ),
            wavs_types::Trigger::BlockInterval {
                chain,
                n_blocks,
                start_block,
                end_block,
            } => aggregator_service::Trigger::BlockInterval(
                aggregator_service::TriggerBlockInterval {
                    chain: chain.to_string(),
                    n_blocks: n_blocks.into(),
                    start_block: start_block.map(Into::into),
                    end_block: end_block.map(Into::into),
                },
            ),
            wavs_types::Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => aggregator_service::Trigger::Cron(aggregator_service::TriggerCron {
                schedule: schedule.to_string(),
                start_time: start_time.map(Into::into),
                end_time: end_time.map(Into::into),
            }),
        })
    }
}

impl TryFrom<layer_climb::prelude::Address> for aggregator_chain::CosmosAddress {
    type Error = anyhow::Error;

    fn try_from(address: layer_climb::prelude::Address) -> Result<Self, Self::Error> {
        match address {
            layer_climb::prelude::Address::Cosmos(addr) => Ok(Self {
                bech32_addr: addr.to_string(),
                prefix_len: addr.prefix().len() as u32,
            }),
            _ => Err(anyhow::anyhow!("Not a cosmos address")),
        }
    }
}

impl From<wavs_types::Timestamp> for aggregator_core::Timestamp {
    fn from(timestamp: wavs_types::Timestamp) -> Self {
        aggregator_core::Timestamp {
            nanos: timestamp.as_nanos(),
        }
    }
}

impl From<wavs_types::Duration> for aggregator_core::Duration {
    fn from(duration: wavs_types::Duration) -> Self {
        aggregator_core::Duration {
            secs: duration.secs,
        }
    }
}

impl From<aggregator_core::Duration> for wavs_types::Duration {
    fn from(duration: aggregator_core::Duration) -> Self {
        wavs_types::Duration {
            secs: duration.secs,
        }
    }
}

impl From<wavs_types::AggregatorAction> for aggregator_types::AggregatorAction {
    fn from(action: wavs_types::AggregatorAction) -> Self {
        match action {
            wavs_types::AggregatorAction::Submit(submit) => {
                aggregator_types::AggregatorAction::Submit(aggregator_types::SubmitAction {
                    chain: submit.chain,
                    contract_address: aggregator_chain::EvmAddress {
                        raw_bytes: submit.contract_address,
                    },
                    gas_price: submit.gas_price.map(|x| x.into()),
                })
            }
            wavs_types::AggregatorAction::Timer(timer) => {
                aggregator_types::AggregatorAction::Timer(aggregator_types::TimerAction {
                    delay: timer.delay.into(),
                })
            }
        }
    }
}

impl From<aggregator_types::AggregatorAction> for wavs_types::AggregatorAction {
    fn from(action: aggregator_types::AggregatorAction) -> Self {
        match action {
            aggregator_types::AggregatorAction::Submit(submit) => {
                wavs_types::AggregatorAction::Submit(wavs_types::SubmitAction {
                    chain: submit.chain,
                    contract_address: submit.contract_address.raw_bytes,
                    gas_price: submit.gas_price.map(|x| x.into()),
                })
            }
            aggregator_types::AggregatorAction::Timer(timer) => {
                wavs_types::AggregatorAction::Timer(wavs_types::TimerAction {
                    delay: timer.delay.into(),
                })
            }
        }
    }
}

impl From<utils::config::CosmosChainConfig> for aggregator_chain::CosmosChainConfig {
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

impl From<utils::config::EvmChainConfig> for aggregator_chain::EvmChainConfig {
    fn from(config: utils::config::EvmChainConfig) -> Self {
        Self {
            chain_id: config.chain_id.to_string(),
            ws_endpoints: config.ws_endpoints,
            http_endpoint: config.http_endpoint,
        }
    }
}
