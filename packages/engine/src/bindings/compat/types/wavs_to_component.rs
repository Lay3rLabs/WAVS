use crate::bindings::world::wavs::types::{
    chain as component_chain,
    service as component_service,
    core as component_core,
};

impl From<utils::config::CosmosChainConfig> for component_chain::CosmosChainConfig {
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

impl From<utils::config::EvmChainConfig> for component_chain::EvmChainConfig {
    fn from(config: utils::config::EvmChainConfig) -> Self {
        Self {
            chain_id: config.chain_id,
            ws_endpoint: config.ws_endpoint,
            http_endpoint: config.http_endpoint,
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
            wavs_types::ServiceManager::Evm {
                chain_name,
                address,
            } => component_service::ServiceManager::Evm(component_service::EvmManager {
                chain_name: chain_name.to_string(),
                address: address.into(),
            }),
        }
    }
}

impl TryFrom<wavs_types::Trigger> for component_service::Trigger {
    type Error = anyhow::Error;

    fn try_from(src: wavs_types::Trigger) -> Result<Self, Self::Error> {
        Ok(match src {
            wavs_types::Trigger::CosmosContractEvent {
                address,
                chain_name,
                event_type,
            } => component_service::Trigger::CosmosContractEvent(
                component_service::TriggerCosmosContractEvent {
                    address: address.try_into()?,
                    chain_name: chain_name.to_string(),
                    event_type,
                },
            ),
            wavs_types::Trigger::EvmContractEvent {
                address,
                chain_name,
                event_hash,
            } => component_service::Trigger::EvmContractEvent(
                component_service::TriggerEvmContractEvent {
                    address: address.into(),
                    chain_name: chain_name.to_string(),
                    event_hash: event_hash.as_slice().to_vec(),
                },
            ),
            wavs_types::Trigger::BlockInterval {
                chain_name,
                n_blocks,
                start_block,
                end_block,
            } => {
                component_service::Trigger::BlockInterval(component_service::TriggerBlockInterval {
                    chain_name: chain_name.to_string(),
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

impl From<wavs_types::Timestamp> for component_core::Timestamp {
    fn from(src: wavs_types::Timestamp) -> Self {
        component_core::Timestamp {
            nanos: src.as_nanos(),
        }
    }
}

impl TryFrom<layer_climb::prelude::Address> for component_chain::CosmosAddress {
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

impl From<utils::config::CosmosChainConfig> for crate::bindings::world::host::CosmosChainConfig {
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

impl From<utils::config::EvmChainConfig> for crate::bindings::world::host::EvmChainConfig {
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
            wavs_types::ComponentSource::Download { url, digest } => {
                component_service::ComponentSource::Download(
                    component_service::ComponentSourceDownload {
                        url: url.to_string(),
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
                evm_contracts,
            } => component_service::Submit::Aggregator(component_service::AggregatorSubmit {
                url,
                component: component.map(|c| (*c).into()),
                evm_contracts: evm_contracts
                    .map(|contracts| contracts.into_iter().map(|c| c.into()).collect()),
            }),
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
            chain_name: src.chain_name.to_string(),
            address: src.address.into(),
            max_gas: src.max_gas,
        }
    }
}