use crate::bindings::aggregator::world::wavs::{
    aggregator::aggregator::{
        Envelope as WitEnvelope, EnvelopeSignature as WitEnvelopeSignature, Packet as WitPacket,
        Secp256k1Signature as WitSecp256k1Signature,
    },
    types::{
        chain::{CosmosAddress as WitCosmosAddress, EvmAddress as WitEvmAddress},
        core::Timestamp as WitTimestamp,
        service::{
            AggregatorSubmit as WitAggregatorSubmit,
            AllowedHostPermission as WitAllowedHostPermission, Component as WitComponent,
            ComponentSource as WitComponentSource,
            ComponentSourceDownload as WitComponentSourceDownload,
            EvmContractSubmission as WitEvmContractSubmission, EvmManager as WitEvmManager,
            Permissions as WitPermissions, Registry as WitRegistry, Service as WitService,
            ServiceManager as WitServiceManager, ServiceStatus as WitServiceStatus,
            Submit as WitSubmit, Trigger as WitTrigger,
            TriggerBlockInterval as WitTriggerBlockInterval,
            TriggerCosmosContractEvent as WitTriggerCosmosContractEvent,
            TriggerCron as WitTriggerCron, TriggerEvmContractEvent as WitTriggerEvmContractEvent,
            Workflow as WitWorkflow,
        },
    },
};
use wavs_types::{Envelope, EnvelopeSignature, Packet};

impl TryFrom<Packet> for WitPacket {
    type Error = anyhow::Error;

    fn try_from(packet: Packet) -> Result<Self, Self::Error> {
        Ok(WitPacket {
            service: packet.service.try_into()?,
            workflow_id: packet.workflow_id.to_string(),
            envelope: packet.envelope.into(),
            signature: packet.signature.into(),
        })
    }
}

impl TryFrom<wavs_types::Service> for WitService {
    type Error = anyhow::Error;

    fn try_from(service: wavs_types::Service) -> Result<Self, Self::Error> {
        Ok(WitService {
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

impl TryFrom<wavs_types::Workflow> for WitWorkflow {
    type Error = anyhow::Error;

    fn try_from(workflow: wavs_types::Workflow) -> Result<Self, Self::Error> {
        Ok(WitWorkflow {
            trigger: workflow.trigger.try_into()?,
            component: workflow.component.into(),
            submit: workflow.submit.into(),
        })
    }
}

impl From<wavs_types::ServiceStatus> for WitServiceStatus {
    fn from(status: wavs_types::ServiceStatus) -> Self {
        match status {
            wavs_types::ServiceStatus::Active => WitServiceStatus::Active,
            wavs_types::ServiceStatus::Paused => WitServiceStatus::Paused,
        }
    }
}

impl From<wavs_types::ServiceManager> for WitServiceManager {
    fn from(manager: wavs_types::ServiceManager) -> Self {
        match manager {
            wavs_types::ServiceManager::Evm {
                chain_name,
                address,
            } => WitServiceManager::Evm(WitEvmManager {
                chain_name: chain_name.to_string(),
                address: WitEvmAddress {
                    raw_bytes: address.to_vec(),
                },
            }),
        }
    }
}

impl From<alloy_primitives::Address> for WitEvmAddress {
    fn from(address: alloy_primitives::Address) -> Self {
        WitEvmAddress {
            raw_bytes: address.to_vec(),
        }
    }
}

impl From<Envelope> for WitEnvelope {
    fn from(envelope: Envelope) -> Self {
        WitEnvelope {
            event_id: envelope.eventId.to_vec(),
            ordering: envelope.ordering.to_vec(),
            payload: envelope.payload.to_vec(),
        }
    }
}

impl From<EnvelopeSignature> for WitEnvelopeSignature {
    fn from(signature: EnvelopeSignature) -> Self {
        match signature {
            EnvelopeSignature::Secp256k1(sig) => {
                WitEnvelopeSignature::Secp256k1(WitSecp256k1Signature {
                    signature_data: sig.as_bytes().to_vec(),
                })
            }
        }
    }
}

impl From<wavs_types::Component> for WitComponent {
    fn from(component: wavs_types::Component) -> Self {
        WitComponent {
            source: component.source.into(),
            permissions: component.permissions.into(),
            fuel_limit: component.fuel_limit,
            time_limit_seconds: component.time_limit_seconds,
            config: component.config.into_iter().collect(),
            env_keys: component.env_keys.into_iter().collect(),
        }
    }
}

impl From<wavs_types::ComponentSource> for WitComponentSource {
    fn from(source: wavs_types::ComponentSource) -> Self {
        match source {
            wavs_types::ComponentSource::Digest(digest) => {
                WitComponentSource::Digest(digest.to_string())
            }
            wavs_types::ComponentSource::Download { url, digest } => {
                WitComponentSource::Download(WitComponentSourceDownload {
                    url: url.to_string(),
                    digest: digest.to_string(),
                })
            }
            wavs_types::ComponentSource::Registry { registry } => {
                WitComponentSource::Registry(registry.into())
            }
        }
    }
}

impl From<wavs_types::Registry> for WitRegistry {
    fn from(registry: wavs_types::Registry) -> Self {
        WitRegistry {
            digest: registry.digest.to_string(),
            domain: registry.domain,
            version: registry.version.map(|v| v.to_string()),
            pkg: registry.package.to_string(),
        }
    }
}

impl From<wavs_types::Permissions> for WitPermissions {
    fn from(permissions: wavs_types::Permissions) -> Self {
        WitPermissions {
            allowed_http_hosts: permissions.allowed_http_hosts.into(),
            file_system: permissions.file_system,
        }
    }
}

impl From<wavs_types::AllowedHostPermission> for WitAllowedHostPermission {
    fn from(permission: wavs_types::AllowedHostPermission) -> Self {
        match permission {
            wavs_types::AllowedHostPermission::All => WitAllowedHostPermission::All,
            wavs_types::AllowedHostPermission::None => WitAllowedHostPermission::None,
            wavs_types::AllowedHostPermission::Only(hosts) => WitAllowedHostPermission::Only(hosts),
        }
    }
}

impl From<wavs_types::Submit> for WitSubmit {
    fn from(submit: wavs_types::Submit) -> Self {
        match submit {
            wavs_types::Submit::None => WitSubmit::None,
            wavs_types::Submit::Aggregator {
                url,
                component,
                evm_contracts,
            } => WitSubmit::Aggregator(WitAggregatorSubmit {
                url,
                component: component.map(|c| (*c).into()),
                evm_contracts: evm_contracts
                    .map(|contracts| contracts.into_iter().map(|c| c.into()).collect()),
            }),
        }
    }
}

impl From<wavs_types::EvmContractSubmission> for WitEvmContractSubmission {
    fn from(submission: wavs_types::EvmContractSubmission) -> Self {
        WitEvmContractSubmission {
            chain_name: submission.chain_name.to_string(),
            address: submission.address.into(),
            max_gas: submission.max_gas,
        }
    }
}

impl TryFrom<wavs_types::Trigger> for WitTrigger {
    type Error = anyhow::Error;

    fn try_from(trigger: wavs_types::Trigger) -> Result<Self, Self::Error> {
        Ok(match trigger {
            wavs_types::Trigger::Manual => WitTrigger::Manual,
            wavs_types::Trigger::EvmContractEvent {
                address,
                chain_name,
                event_hash,
            } => WitTrigger::EvmContractEvent(WitTriggerEvmContractEvent {
                address: address.into(),
                chain_name: chain_name.to_string(),
                event_hash: event_hash.as_slice().to_vec(),
            }),
            wavs_types::Trigger::CosmosContractEvent {
                address,
                chain_name,
                event_type,
            } => WitTrigger::CosmosContractEvent(WitTriggerCosmosContractEvent {
                address: address.try_into()?,
                chain_name: chain_name.to_string(),
                event_type,
            }),
            wavs_types::Trigger::BlockInterval {
                chain_name,
                n_blocks,
                start_block,
                end_block,
            } => WitTrigger::BlockInterval(WitTriggerBlockInterval {
                chain_name: chain_name.to_string(),
                n_blocks: n_blocks.into(),
                start_block: start_block.map(Into::into),
                end_block: end_block.map(Into::into),
            }),
            wavs_types::Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => WitTrigger::Cron(WitTriggerCron {
                schedule: schedule.to_string(),
                start_time: start_time.map(Into::into),
                end_time: end_time.map(Into::into),
            }),
        })
    }
}

impl TryFrom<layer_climb::prelude::Address> for WitCosmosAddress {
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

impl From<wavs_types::Timestamp> for WitTimestamp {
    fn from(timestamp: wavs_types::Timestamp) -> Self {
        WitTimestamp {
            nanos: timestamp.as_nanos(),
        }
    }
}
