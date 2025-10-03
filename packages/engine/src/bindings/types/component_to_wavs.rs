use std::{collections::BTreeMap, str::FromStr};

use iri_string::types::UriString;
use wavs_types::WorkflowId;

use crate::{
    bindings::operator::world::wavs::operator::output as component_output,
    bindings::operator::world::wavs::types::chain as component_chain,
    bindings::operator::world::wavs::types::core as component_core,
    bindings::operator::world::wavs::types::service as component_service,
};

impl TryFrom<component_service::Trigger> for wavs_types::Trigger {
    type Error = anyhow::Error;

    fn try_from(src: component_service::Trigger) -> Result<Self, Self::Error> {
        Ok(match src {
            component_service::Trigger::CosmosContractEvent(source) => {
                wavs_types::Trigger::CosmosContractEvent {
                    address: source.address.into(),
                    chain: source.chain.parse()?,
                    event_type: source.event_type,
                }
            }
            component_service::Trigger::EvmContractEvent(source) => {
                wavs_types::Trigger::EvmContractEvent {
                    address: source.address.into(),
                    chain: source.chain.parse()?,
                    event_hash: source.event_hash.try_into()?,
                }
            }
            component_service::Trigger::BlockInterval(source) => {
                wavs_types::Trigger::BlockInterval {
                    chain: source.chain.parse()?,
                    n_blocks: source.n_blocks.try_into()?,
                    start_block: source.start_block.map(TryInto::try_into).transpose()?,
                    end_block: source.end_block.map(TryInto::try_into).transpose()?,
                }
            }
            component_service::Trigger::Manual => wavs_types::Trigger::Manual,
            component_service::Trigger::Cron(source) => wavs_types::Trigger::Cron {
                schedule: source.schedule,
                start_time: source.start_time.map(Into::into),
                end_time: source.end_time.map(Into::into),
            },
        })
    }
}

impl From<component_chain::CosmosAddress> for layer_climb::prelude::Address {
    fn from(address: component_chain::CosmosAddress) -> Self {
        layer_climb::prelude::Address::Cosmos {
            bech32_addr: address.bech32_addr,
            prefix_len: address.prefix_len as usize,
        }
    }
}

impl From<component_chain::EvmAddress> for layer_climb::prelude::Address {
    fn from(address: component_chain::EvmAddress) -> Self {
        layer_climb::prelude::Address::Evm(
            alloy_primitives::Address::from_slice(&address.raw_bytes).into(),
        )
    }
}

impl From<component_chain::EvmAddress> for alloy_primitives::Address {
    fn from(address: component_chain::EvmAddress) -> Self {
        Self::from_slice(&address.raw_bytes)
    }
}

impl From<component_core::Timestamp> for wavs_types::Timestamp {
    fn from(src: component_core::Timestamp) -> Self {
        wavs_types::Timestamp::from_nanos(src.nanos)
    }
}

impl TryFrom<component_service::Service> for wavs_types::Service {
    type Error = anyhow::Error;

    fn try_from(src: component_service::Service) -> Result<Self, Self::Error> {
        Ok(Self {
            name: src.name,
            workflows: src
                .workflows
                .into_iter()
                .map(|(workflow_id, workflow)| {
                    let workflow_id: WorkflowId = workflow_id.parse()?;
                    let workflow: wavs_types::Workflow = workflow.try_into()?;
                    Ok((workflow_id, workflow))
                })
                .collect::<anyhow::Result<BTreeMap<WorkflowId, wavs_types::Workflow>>>()?,
            status: src.status.into(),
            manager: src.manager.try_into()?,
        })
    }
}

impl TryFrom<component_service::Workflow> for wavs_types::Workflow {
    type Error = anyhow::Error;

    fn try_from(src: component_service::Workflow) -> Result<Self, Self::Error> {
        Ok(Self {
            trigger: src.trigger.try_into()?,
            component: src.component.try_into()?,
            submit: src.submit.into(),
        })
    }
}

impl TryFrom<component_service::Component> for wavs_types::Component {
    type Error = anyhow::Error;

    fn try_from(src: component_service::Component) -> Result<Self, Self::Error> {
        Ok(Self {
            source: src.source.try_into()?,
            permissions: src.permissions.into(),
            fuel_limit: src.fuel_limit,
            time_limit_seconds: src.time_limit_seconds,
            config: src.config.into_iter().collect(),
            env_keys: src.env_keys.into_iter().collect(),
        })
    }
}

impl TryFrom<component_service::ComponentSource> for wavs_types::ComponentSource {
    type Error = anyhow::Error;

    fn try_from(src: component_service::ComponentSource) -> Result<Self, Self::Error> {
        Ok(match src {
            component_service::ComponentSource::Digest(digest) => {
                wavs_types::ComponentSource::Digest(wavs_types::ComponentDigest::from_str(&digest)?)
            }
            component_service::ComponentSource::Download(download) => {
                wavs_types::ComponentSource::Download {
                    uri: UriString::try_from(download.url)?,
                    digest: wavs_types::ComponentDigest::from_str(&download.digest)?,
                }
            }
            component_service::ComponentSource::Registry(registry) => {
                wavs_types::ComponentSource::Registry {
                    registry: registry.try_into()?,
                }
            }
        })
    }
}

impl TryFrom<component_service::Registry> for wavs_types::Registry {
    type Error = anyhow::Error;

    fn try_from(src: component_service::Registry) -> Result<Self, Self::Error> {
        Ok(Self {
            digest: wavs_types::ComponentDigest::from_str(&src.digest)?,
            domain: src.domain,
            version: src.version.map(|v| v.parse()).transpose()?,
            package: src.pkg.try_into()?,
        })
    }
}

impl From<component_service::Permissions> for wavs_types::Permissions {
    fn from(src: component_service::Permissions) -> Self {
        Self {
            allowed_http_hosts: src.allowed_http_hosts.into(),
            file_system: src.file_system,
        }
    }
}

impl From<component_service::AllowedHostPermission> for wavs_types::AllowedHostPermission {
    fn from(src: component_service::AllowedHostPermission) -> Self {
        match src {
            component_service::AllowedHostPermission::All => wavs_types::AllowedHostPermission::All,
            component_service::AllowedHostPermission::None => {
                wavs_types::AllowedHostPermission::None
            }
            component_service::AllowedHostPermission::Only(hosts) => {
                wavs_types::AllowedHostPermission::Only(hosts.into_iter().collect())
            }
        }
    }
}

impl From<component_service::ServiceStatus> for wavs_types::ServiceStatus {
    fn from(src: component_service::ServiceStatus) -> Self {
        match src {
            component_service::ServiceStatus::Active => wavs_types::ServiceStatus::Active,
            component_service::ServiceStatus::Paused => wavs_types::ServiceStatus::Paused,
        }
    }
}

impl TryFrom<component_service::ServiceManager> for wavs_types::ServiceManager {
    type Error = anyhow::Error;

    fn try_from(src: component_service::ServiceManager) -> Result<Self, Self::Error> {
        Ok(match src {
            component_service::ServiceManager::Evm(evm) => wavs_types::ServiceManager::Evm {
                chain: evm.chain.parse()?,
                address: evm.address.into(),
            },
        })
    }
}

impl From<component_service::Submit> for wavs_types::Submit {
    fn from(src: component_service::Submit) -> Self {
        match src {
            component_service::Submit::None => wavs_types::Submit::None,
            component_service::Submit::Aggregator(component_service::AggregatorSubmit {
                url,
                component,
                signature_kind,
            }) => wavs_types::Submit::Aggregator {
                url,
                component: Box::new(component.try_into().unwrap()),
                signature_kind: signature_kind.into(),
            },
        }
    }
}

impl From<component_service::SignatureKind> for wavs_types::SignatureKind {
    fn from(src: component_service::SignatureKind) -> Self {
        Self {
            algorithm: src.algorithm.into(),
            prefix: src.prefix.map(Into::into),
        }
    }
}

impl From<component_service::SignatureAlgorithm> for wavs_types::SignatureAlgorithm {
    fn from(src: component_service::SignatureAlgorithm) -> Self {
        match src {
            component_service::SignatureAlgorithm::Secp256k1 => {
                wavs_types::SignatureAlgorithm::Secp256k1
            }
        }
    }
}

impl From<component_service::SignaturePrefix> for wavs_types::SignaturePrefix {
    fn from(src: component_service::SignaturePrefix) -> Self {
        match src {
            component_service::SignaturePrefix::Eip191 => wavs_types::SignaturePrefix::Eip191,
        }
    }
}

impl TryFrom<component_service::Aggregator> for wavs_types::Aggregator {
    type Error = anyhow::Error;

    fn try_from(src: component_service::Aggregator) -> Result<Self, Self::Error> {
        Ok(match src {
            component_service::Aggregator::Evm(evm) => wavs_types::Aggregator::Evm(evm.try_into()?),
        })
    }
}

impl TryFrom<component_service::EvmContractSubmission> for wavs_types::EvmContractSubmission {
    type Error = anyhow::Error;

    fn try_from(src: component_service::EvmContractSubmission) -> Result<Self, Self::Error> {
        Ok(Self {
            chain: src.chain.parse()?,
            address: src.address.into(),
            max_gas: src.max_gas,
        })
    }
}

impl From<component_output::WasmResponse> for wavs_types::WasmResponse {
    fn from(src: component_output::WasmResponse) -> Self {
        Self {
            payload: src.payload,
            ordering: src.ordering,
        }
    }
}
