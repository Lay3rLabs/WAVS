use crate::bindings::aggregator::world::wavs::{
    aggregator::aggregator as aggregator_types, types::chain as aggregator_chain,
    types::core as aggregator_core, types::service as aggregator_service,
};
use wavs_types::{Envelope, EnvelopeSignature, Packet};

impl TryFrom<Packet> for aggregator_types::Packet {
    type Error = anyhow::Error;

    fn try_from(packet: Packet) -> Result<Self, Self::Error> {
        Ok(aggregator_types::Packet {
            service: packet.service.try_into()?,
            workflow_id: packet.workflow_id.to_string(),
            envelope: packet.envelope.into(),
            signature: packet.signature.into(),
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
            wavs_types::ServiceManager::Evm {
                chain_name,
                address,
            } => aggregator_service::ServiceManager::Evm(aggregator_service::EvmManager {
                chain_name: chain_name.to_string(),
                address: aggregator_chain::EvmAddress {
                    raw_bytes: address.to_vec(),
                },
            }),
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

impl From<Envelope> for aggregator_types::Envelope {
    fn from(envelope: Envelope) -> Self {
        aggregator_types::Envelope {
            event_id: envelope.eventId.to_vec(),
            ordering: envelope.ordering.to_vec(),
            payload: envelope.payload.to_vec(),
        }
    }
}

impl From<EnvelopeSignature> for aggregator_types::EnvelopeSignature {
    fn from(signature: EnvelopeSignature) -> Self {
        match signature {
            EnvelopeSignature::Secp256k1(sig) => aggregator_types::EnvelopeSignature::Secp256k1(
                aggregator_types::Secp256k1Signature {
                    signature_data: sig.as_bytes().to_vec(),
                },
            ),
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
            wavs_types::ComponentSource::Download { url, digest } => {
                aggregator_service::ComponentSource::Download(
                    aggregator_service::ComponentSourceDownload {
                        url: url.to_string(),
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
            wavs_types::Submit::Aggregator { url, component } => {
                aggregator_service::Submit::Aggregator(aggregator_service::AggregatorSubmit {
                    url,
                    component: (*component).into(),
                })
            }
        }
    }
}

impl From<wavs_types::EvmContractSubmission> for aggregator_service::EvmContractSubmission {
    fn from(submission: wavs_types::EvmContractSubmission) -> Self {
        aggregator_service::EvmContractSubmission {
            chain_name: submission.chain_name.to_string(),
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
                chain_name,
                event_hash,
            } => aggregator_service::Trigger::EvmContractEvent(
                aggregator_service::TriggerEvmContractEvent {
                    address: address.into(),
                    chain_name: chain_name.to_string(),
                    event_hash: event_hash.as_slice().to_vec(),
                },
            ),
            wavs_types::Trigger::CosmosContractEvent {
                address,
                chain_name,
                event_type,
            } => aggregator_service::Trigger::CosmosContractEvent(
                aggregator_service::TriggerCosmosContractEvent {
                    address: address.try_into()?,
                    chain_name: chain_name.to_string(),
                    event_type,
                },
            ),
            wavs_types::Trigger::BlockInterval {
                chain_name,
                n_blocks,
                start_block,
                end_block,
            } => aggregator_service::Trigger::BlockInterval(
                aggregator_service::TriggerBlockInterval {
                    chain_name: chain_name.to_string(),
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
        let (bech32_addr, prefix_len) = match address {
            layer_climb::prelude::Address::Cosmos {
                bech32_addr,
                prefix_len,
            } => (bech32_addr, prefix_len),
            _ => return Err(anyhow::anyhow!("Not a cosmos address")),
        };

        Ok(Self {
            bech32_addr,
            prefix_len: prefix_len as u32,
        })
    }
}

impl From<wavs_types::Timestamp> for aggregator_core::Timestamp {
    fn from(timestamp: wavs_types::Timestamp) -> Self {
        aggregator_core::Timestamp {
            nanos: timestamp.as_nanos(),
        }
    }
}
