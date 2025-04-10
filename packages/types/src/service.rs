use alloy::primitives::LogData;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use wasm_pkg_common::package::{PackageRef, Version};

use crate::{ByteArray, Digest, Timestamp};

use super::{ChainName, ServiceID, WorkflowID};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Service {
    // Public identifier. Must be unique for all services
    pub id: ServiceID,

    /// This is any utf-8 string, for human-readable display.
    pub name: String,

    /// We support multiple workflows in one service with unique service-scoped IDs.
    pub workflows: BTreeMap<WorkflowID, Workflow>,

    pub status: ServiceStatus,

    pub manager: ServiceManager,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceManager {
    Ethereum {
        chain_name: ChainName,
        address: alloy::primitives::Address,
    },
}

impl ServiceManager {
    pub fn chain_name(&self) -> &ChainName {
        match self {
            ServiceManager::Ethereum { chain_name, .. } => chain_name,
        }
    }

    pub fn eth_address_unchecked(&self) -> alloy::primitives::Address {
        match self {
            ServiceManager::Ethereum { address, .. } => *address,
        }
    }
}

impl Service {
    pub fn new_simple(
        id: ServiceID,
        name: Option<String>,
        trigger: Trigger,
        source: ComponentSource,
        submit: Submit,
        manager: ServiceManager,
    ) -> Self {
        let workflow_id = WorkflowID::default();

        let workflow = Workflow {
            trigger,
            component: Component::new(source),
            submit,
            aggregator: None,
        };

        let workflows = BTreeMap::from([(workflow_id, workflow)]);

        Self {
            name: name.unwrap_or_else(|| id.to_string()),
            id,
            workflows,
            status: ServiceStatus::Active,
            manager,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Component {
    pub source: ComponentSource,

    // What permissions this component has.
    // These are currently not enforced, you can pass in Default::default() for now
    pub permissions: Permissions,

    /// The maximum amount of compute metering to allow for a single component execution
    /// If not supplied, will be `Workflow::DEFAULT_FUEL_LIMIT`
    pub fuel_limit: Option<u64>,

    /// The maximum amount of time to allow for a single component execution, in seconds
    /// If not supplied, default will be `Workflow::DEFAULT_TIME_LIMIT_SECONDS`
    pub time_limit_seconds: Option<u64>,

    /// Key-value pairs that are accessible in the components via host bindings.
    pub config: BTreeMap<String, String>,

    /// External env variable keys to be read from the system host on execute (i.e. API keys).
    /// Must be prefixed with `WAVS_ENV_`.
    pub env_keys: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentSource {
    /// The wasm bytecode provided at fixed url, digest provided to ensure no tampering
    Download { url: String, digest: Digest },
    /// The wasm bytecode downloaded from a standard registry, digest provided to ensure no tampering
    Registry { registry: Registry },
    /// An already deployed component
    Digest(Digest),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Registry {
    pub digest: Digest,
    /// Optional domain to use for a registry (such as ghcr.io)
    /// if default of wa.dev (or whatever wavs uses in the future)
    /// is not desired by user
    pub domain: Option<String>,
    /// Optional semver value, if absent then latest is used
    pub version: Option<Version>,
    /// Package identifier of form <namespace>:<packagename>
    pub package: PackageRef,
}

impl ComponentSource {
    pub fn digest(&self) -> &Digest {
        match self {
            ComponentSource::Download { digest, .. } => digest,
            ComponentSource::Registry { registry } => &registry.digest,
            ComponentSource::Digest(digest) => digest,
        }
    }
}

// FIXME: happy for a better name.
/// This captures the triggers we listen to, the components we run, and how we submit the result
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Workflow {
    /// The trigger that fires this workflow
    pub trigger: Trigger,

    /// The component to run when the trigger fires
    pub component: Component,

    /// How to submit the result of the component.
    pub submit: Submit,

    /// If submit is `Submit::Aggregator`, this is
    /// the required data for the aggregator to submit this workflow
    pub aggregator: Option<Aggregator>,
}

impl Workflow {
    pub const DEFAULT_FUEL_LIMIT: u64 = 100_000_000;
    pub const DEFAULT_TIME_LIMIT_SECONDS: u64 = 30;
}

// The TriggerManager reacts to these triggers
#[derive(Hash, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    // A contract that emits an event
    CosmosContractEvent {
        address: layer_climb_address::Address,
        chain_name: ChainName,
        event_type: String,
    },
    EthContractEvent {
        address: alloy::primitives::Address,
        chain_name: ChainName,
        event_hash: ByteArray<32>,
    },
    BlockInterval {
        chain_name: ChainName,
        n_blocks: NonZeroU32,
    },
    Cron {
        /// A cron expression defining the schedule for execution.
        schedule: String,
        /// Optional start time (timestamp in seconds) indicating when the schedule begins.
        start_time: Option<Timestamp>,
        /// Optional end time (timestamp in seconds) indicating when the schedule ends.
        end_time: Option<Timestamp>,
    },
    // not a real trigger, just for testing
    Manual,
}

/// The data that came from the trigger and is passed to the component after being converted into the WIT-friendly type
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum TriggerData {
    CosmosContractEvent {
        /// The address of the contract that emitted the event
        contract_address: layer_climb_address::Address,
        /// The name of the chain where the event was emitted
        chain_name: ChainName,
        /// The data that was emitted by the contract
        event: cosmwasm_std::Event,
        /// The block height where the event was emitted
        block_height: u64,
    },
    EthContractEvent {
        /// The address of the contract that emitted the event
        contract_address: alloy::primitives::Address,
        /// The name of the chain where the event was emitted
        chain_name: ChainName,
        /// The raw event log
        log: LogData,
        /// The block height where the event was emitted
        block_height: u64,
    },
    BlockInterval {
        /// The name of the chain where the blocks are checked
        chain_name: ChainName,
        /// The block height where the event was emitted
        block_height: u64,
    },
    Cron {
        /// The trigger time
        trigger_time: Timestamp,
    },
    Raw(Vec<u8>),
}

impl TriggerData {
    pub fn new_raw(data: impl AsRef<[u8]>) -> Self {
        TriggerData::Raw(data.as_ref().to_vec())
    }
}

/// A bundle of the trigger and the associated data needed to take action on it
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, bincode::Decode, bincode::Encode)]
pub struct TriggerAction {
    #[bincode(with_serde)]
    /// Identify which trigger this came from
    pub config: TriggerConfig,

    #[bincode(with_serde)]
    /// The data that came from the trigger
    pub data: TriggerData,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
// Trigger with metadata so it can be identified in relation to services and workflows
pub struct TriggerConfig {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub trigger: Trigger,
}

// TODO - rename this? Trigger is a noun, Submit is a verb.. feels a bit weird
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Submit {
    // useful for when the component just does something with its own state
    None,
    Aggregator {
        /// The aggregator endpoint
        url: String,
    },
    /// Service handler directly
    EthereumContract(EthereumContractSubmission),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Aggregator {
    Ethereum(EthereumContractSubmission),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct EthereumContractSubmission {
    pub chain_name: ChainName,
    /// Should be an IWavsServiceHandler contract
    pub address: alloy::primitives::Address,
    /// max gas for the submission
    /// with an aggregator, that will be for all the signed envelopes combined
    /// without an aggregator, it's just the single signed envelope
    pub max_gas: Option<u64>,
}

impl EthereumContractSubmission {
    pub fn new(
        chain_name: ChainName,
        address: alloy::primitives::Address,
        max_gas: Option<u64>,
    ) -> Self {
        Self {
            chain_name,
            address,
            max_gas,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Active,
    // we could have more like Stopped, Failed, Cooldown, etc.
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default, rename_all = "snake_case")]
#[derive(Default)]
pub struct Permissions {
    /// If it can talk to http hosts on the network
    pub allowed_http_hosts: AllowedHostPermission,
    /// If it can write to it's own local directory in the filesystem
    pub file_system: bool,
}

// TODO: remove / change defaults?

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllowedHostPermission {
    All,
    Only(Vec<String>),
    #[default]
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default, rename_all = "snake_case")]
#[derive(Default)]
pub struct WasmResponse {
    pub payload: Vec<u8>,
    pub ordering: Option<u64>,
}

// TODO - these shouldn't be needed in main code... gate behind `debug_assertions`
// will need to go through use-cases of `test-utils`, maybe move into layer-tests or something
mod test_ext {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use crate::{id::ChainName, ByteArray, ComponentSource, IDError, ServiceID, WorkflowID};

    use super::{Component, EthereumContractSubmission, Submit, Trigger, TriggerConfig};

    impl Submit {
        pub fn eth_contract(
            chain_name: ChainName,
            address: alloy::primitives::Address,
            max_gas: Option<u64>,
        ) -> Submit {
            Submit::EthereumContract(EthereumContractSubmission::new(
                chain_name, address, max_gas,
            ))
        }
    }

    impl Component {
        pub fn new(source: ComponentSource) -> Component {
            Self {
                source,
                permissions: Default::default(),
                fuel_limit: None,
                time_limit_seconds: None,
                config: BTreeMap::new(),
                env_keys: vec![],
            }
        }
    }

    impl Trigger {
        pub fn cosmos_contract_event(
            address: layer_climb_address::Address,
            chain_name: impl Into<ChainName>,
            event_type: impl ToString,
        ) -> Self {
            Trigger::CosmosContractEvent {
                address,
                chain_name: chain_name.into(),
                event_type: event_type.to_string(),
            }
        }
        pub fn eth_contract_event(
            address: alloy::primitives::Address,
            chain_name: impl Into<ChainName>,
            event_hash: ByteArray<32>,
        ) -> Self {
            Trigger::EthContractEvent {
                address,
                chain_name: chain_name.into(),
                event_hash,
            }
        }
    }

    impl TriggerConfig {
        pub fn cosmos_contract_event(
            service_id: impl TryInto<ServiceID, Error = IDError>,
            workflow_id: impl TryInto<WorkflowID, Error = IDError>,
            contract_address: layer_climb_address::Address,
            chain_name: impl Into<ChainName>,
            event_type: impl ToString,
        ) -> Result<Self, IDError> {
            Ok(Self {
                service_id: service_id.try_into()?,
                workflow_id: workflow_id.try_into()?,
                trigger: Trigger::cosmos_contract_event(contract_address, chain_name, event_type),
            })
        }

        pub fn eth_contract_event(
            service_id: impl TryInto<ServiceID, Error = IDError>,
            workflow_id: impl TryInto<WorkflowID, Error = IDError>,
            contract_address: alloy::primitives::Address,
            chain_name: impl Into<ChainName>,
            event_hash: ByteArray<32>,
        ) -> Result<Self, IDError> {
            Ok(Self {
                service_id: service_id.try_into()?,
                workflow_id: workflow_id.try_into()?,
                trigger: Trigger::eth_contract_event(contract_address, chain_name, event_hash),
            })
        }

        pub fn block_interval_event(
            service_id: impl TryInto<ServiceID, Error = IDError>,
            workflow_id: impl TryInto<WorkflowID, Error = IDError>,
            chain_name: impl Into<ChainName>,
            n_blocks: NonZeroU32,
        ) -> Result<Self, IDError> {
            Ok(Self {
                service_id: service_id.try_into()?,
                workflow_id: workflow_id.try_into()?,
                trigger: Trigger::BlockInterval {
                    chain_name: chain_name.into(),
                    n_blocks,
                },
            })
        }

        pub fn manual(
            service_id: impl TryInto<ServiceID, Error = IDError>,
            workflow_id: impl TryInto<WorkflowID, Error = IDError>,
        ) -> Result<Self, IDError> {
            Ok(Self {
                service_id: service_id.try_into()?,
                workflow_id: workflow_id.try_into()?,
                trigger: Trigger::Manual,
            })
        }
    }
}
