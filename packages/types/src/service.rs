use std::collections::BTreeMap;

use alloy_primitives::LogData;
use serde::{Deserialize, Serialize};

use crate::{digest::Digest, ByteArray};

use super::{ChainName, ComponentID, ServiceID, TriggerName, WorkflowID};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Service {
    // Public identifier. Must be unique for all services
    pub id: ServiceID,

    /// This is any utf-8 string, for human-readable display.
    pub name: String,

    /// We will supoort multiple components in one service with unique service-scoped IDs. For now, just add one called "default".
    /// This allows clean mapping from backwards-compatible API endpoints.
    pub components: BTreeMap<ComponentID, Component>,

    /// We will support multiple workflows in one service with unique service-scoped IDs. For now, only one called "default".
    /// The workflows reference components by name (for now, always "default").
    pub workflows: BTreeMap<WorkflowID, Workflow>,

    pub status: ServiceStatus,

    pub config: ServiceConfig,
}

impl Service {
    pub fn new_simple(
        id: ServiceID,
        name: Option<String>,
        trigger: Trigger,
        component_digest: Digest,
        submit: Submit,
        config: Option<ServiceConfig>,
    ) -> Self {
        let component_id = ComponentID::default();
        let workflow_id = WorkflowID::default();

        let workflow = Workflow {
            trigger,
            component: component_id,
            submit,
            fuel_limit: None,
        };

        let component = Component {
            wasm: component_digest,
            permissions: Permissions::default(),
        };

        let components = BTreeMap::from([(workflow.component.clone(), component)]);

        let workflows = BTreeMap::from([(workflow_id, workflow)]);

        Self {
            name: name.unwrap_or_else(|| id.to_string()),
            id,
            components,
            workflows,
            status: ServiceStatus::Active,
            config: config.unwrap_or_default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Component {
    pub wasm: Digest,
    // What permissions this component has.
    // These are currently not enforced, you can pass in Default::default() for now
    pub permissions: Permissions,
}

// FIXME: happy for a better name.
/// This captures the triggers we listen to, the components we run, and how we submit the result
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Workflow {
    pub trigger: Trigger,
    /// A reference to which component to run with this data - for now, always "default"
    pub component: ComponentID,
    /// How to submit the result of the component.
    pub submit: Submit,
    /// The maximum amount of compute metering to allow for a single component execution
    /// If not supplied, will be `Workflow::DEFAULT_FUEL_LIMIT`
    pub fuel_limit: Option<u64>,
}

impl Workflow {
    pub const DEFAULT_FUEL_LIMIT: u64 = 100_000_000;
}

// The TriggerManager reacts to these triggers
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    // A contract that emits an event
    CosmosContractEvent {
        address: layer_climb_address::Address,
        chain_name: ChainName,
        event_type: String,
    },
    EthContractEvent {
        address: alloy_primitives::Address,
        chain_name: ChainName,
        event_hash: ByteArray<32>,
    },
    BlockInterval {
        chain_name: ChainName,
        trigger_name: TriggerName,
        n_blocks: u32,
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
        contract_address: alloy_primitives::Address,
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
        /// The unique name of the trigger to distinguish it
        trigger_name: TriggerName,
        /// The block height where the event was emitted
        block_height: u64,
    },
    Raw(Vec<u8>),
}

impl TriggerData {
    pub fn new_raw(data: impl AsRef<[u8]>) -> Self {
        TriggerData::Raw(data.as_ref().to_vec())
    }
}

/// A bundle of the trigger and the associated data needed to take action on it
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TriggerAction {
    /// Identify which trigger this came from
    pub config: TriggerConfig,

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
    // Ethereum Contract which implements the ILayerService interface
    EthereumContract {
        chain_name: ChainName,
        address: alloy_primitives::Address,
        max_gas: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ServiceConfig {
    /// External env variable keys to be read from the system host on execute (i.e. API keys).
    /// Must be prefixed with `WAVS_ENV_`.
    pub host_envs: Vec<String>,
    /// Configuration key-value pairs that are accessible in the components environment.
    /// These config values are public and viewable by anyone.
    /// Components read the values with `std::env::var`, case sensitive & no prefix required.
    /// Values here are viewable by anyone. Use host_envs to set private values.
    pub kv: Vec<(String, String)>,
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

// TODO - these shouldn't be needed in main code... gate behind `debug_assertions`
// will need to go through use-cases of `test-utils`, maybe move into layer-tests or something
mod test_ext {
    use crate::{digest::Digest, id::ChainName, ByteArray, IDError, ServiceID, WorkflowID};

    use super::{Component, Submit, Trigger, TriggerConfig};

    impl Submit {
        pub fn eth_contract(
            chain_name: ChainName,
            address: alloy_primitives::Address,
            max_gas: Option<u64>,
        ) -> Submit {
            Submit::EthereumContract {
                chain_name,
                address,
                max_gas,
            }
        }
    }

    impl Component {
        pub fn new(digest: Digest) -> Component {
            Self {
                wasm: digest,
                permissions: Default::default(),
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
            address: alloy_primitives::Address,
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
            contract_address: alloy_primitives::Address,
            chain_name: impl Into<ChainName>,
            event_hash: ByteArray<32>,
        ) -> Result<Self, IDError> {
            Ok(Self {
                service_id: service_id.try_into()?,
                workflow_id: workflow_id.try_into()?,
                trigger: Trigger::eth_contract_event(contract_address, chain_name, event_hash),
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
