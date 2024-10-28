// This is currently a scratchpad to define some interfaces for the system level.
// It probably should be pulled into multiple files before merging, but I think easier to visualize and review all together first.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::storage::{CAStorage, CAStorageError};
use crate::Digest;

/***
 *
 * High-level system design
 *
 * The main component is the Operator, which can receive "management" calls via the http server
 * to determine its configuration. It works at the level of "Services" which are independent
 * collections of code and triggers that serve one AVS.
 *
 * Principally the Operator manages workflows by the following system:
 *
 * * When the workflow is created, it adds all relevant triggers to the TriggerManager
 * * It continually listens to new results from the TriggerManager, and executes them on the WasmEngine.
 * * When the WasmEngine has produced the result, it submits it to the verifier contract.
 *
 * The TriggerManager has it's own internal runtime and is meant to be able to handle a large number of
 * async network requests. These may be polling or event-driven (websockets), but there are expected to be quite
 * a few network calls and relatively little computation.
 *
 * The WasmEngine stores a large number of wasm components, indexed by their digest.
 * It should be able to quickly execute any of them, via a number of predefined wit component interfaces.
 * We do want to limit the number of wasmtime instances at once. For testing, a simple Mutex around the WasmEngine
 * should demo this. For real usage, we should use some internal threadpool like rayon at set a max number of
 * engines running at once. We may want to make this an async interface?
 *
 * Once the results are calculated, they need to be signed and submitted to the chain (or later to the aggregator).
 * We can do this in the operatator itself, or design a new subsystem for that. (Open to suggestions).
 *
 * I think the biggest question in my head is how to handle all these different runtimes and sync/async assumptions.
 * * Tokio channels is one way (which triggers use as it really matches this fan-in element well) - which allow each side to be either sync or async.
 * * Async code can call sync via `tokio::spawn_blocking`, but we may need some limit on how many such threads can be active at once
 *
 * Currently, I have a strong inclination to use sync code for:
 * * WasmEngine (it seems more stable)
 * * ReDB / KVStore (official recommendation is to wrap it with `tokio::block_in_place` or such if you need it async)
 *
 * And use async code for:
 * * TriggerManager
 * * HTTP Server
 *
 * I think the internal operation of the Operator is my biggest question.
 * Along with how to organize the submission of results.
 * And then how to somehow throttle concurrent access to the WasmEngine.
 *
 ***/

/// This is the highest-level container for the system.
/// The http server can hold this in state and interact with the "management interface".
/// The other components route to each other via this one.
///
/// It uses internal mutability pattern, so we can have multiple references to it.
/// It should implement Send and Sync so it can be used in async code.
pub struct Operator {}

// "management interface"
impl Operator {
    /// Used to install new wasm bytecode into the system.
    /// Either the bytecode is provided directly, or it is downloaded from a URL.
    pub fn add_wasm(&self, _source: WasmSource) -> Result<Digest, OperatorError> {
        todo!();
    }

    pub fn add_service(&self, _service: ServiceDefinition) -> Result<(), OperatorError> {
        todo!();
    }

    pub fn remove_service(&self, _name: String) -> Result<(), OperatorError> {
        todo!();
    }

    pub fn list_services(&self) -> Result<Vec<Service>, OperatorError> {
        todo!();
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WasmSource {
    /// The wasm bytecode is provided directly.
    Bytecode(Vec<u8>),
    /// The wasm bytecode provided at fixed url, digest provided to ensure no tampering
    Download { url: String, digest: Digest },
    /// The wasm bytecode downloaded from a standard registry, digest provided to ensure no tampering
    Registry {
        // TODO: what info do we need here?
        // TODO: can we support some login info for private registries, as env vars in config or something?
        registry: String,
        digest: Digest,
    },
}

/// Information the user provides for the service they want to install.
/// Note: this is similar to the App struct in the old codebase
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDefinition {
    /// This is a limited set of characters, to ensure it can be used in filesystem paths and URLs.
    pub id: ID,
    /// This is any utf-8 string, for human-readable display.
    pub name: String,
    pub component: ComponentDefinition,
    pub workflow: WorkflowDefintion,
    pub testable: Option<bool>,
}

// Question: should we make different public format than internal format?
pub type ComponentDefinition = Component;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub wasm: Digest,
    // What permissions this component has.
    // These are currently not enforced, you can pass in Default::default() for now
    pub permissions: Permissions,
    pub env: Vec<[String; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    /// If it can talk to http hosts on the network
    pub allowed_http_hosts: AllowedHostPermission,
    /// If it can write to it's own local directory in the filesystem
    pub file_system: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum AllowedHostPermission {
    All,
    Only(Vec<String>),
    #[default]
    None,
}

// FIXME: evaluate if we want a different public vs internal type here
pub type WorkflowDefintion = Workflow;

// FIXME: happy for a better name.
/// This captures the triggers we listen to, the components we run, and how we submit the result
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub trigger: Trigger,
    /// A reference to which component to run with this data - for now, always "default"
    pub component: ID,
    /// How to submit the result of the component.
    /// May be unset for eg cron jobs that just update internal state and don't submit anything
    pub submit: Option<Submit>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    // TODO: add this variant later, not for now
    // #[serde(rename_all = "camelCase")]
    // Cron { schedule: String },
    #[serde(rename_all = "camelCase")]
    Queue {
        // FIXME: add some chain name. right now all triggers are on one chain
        task_queue_addr: String,
        /// Frequency in seconds to poll the task queue (doubt this is over 3600 ever, but who knows)
        poll_interval: u32,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Submit {
    /// The hd index of the mnemonic to sign with
    pub hd_index: u32,
    // The address of the verifier contract to submit to
    // Note: To keep the same axum API, the http server can query this from the task queue contract (which is provided)
    // I want to break these hard dependencies internally, so Operator doesn't assume those connections between contracts
    pub verifier_addr: String,
}

/// How we store the service internally.
///
pub struct Service {
    // Public identifier. Must be unique for all services
    pub id: ID,

    /// This is any utf-8 string, for human-readable display.
    pub name: String,

    /// We will supoort multiple components in one service with unique service-scoped IDs. For now, just add one called "default".
    /// This allows clean mapping from backwards-compatible API endpoints.
    pub components: BTreeMap<ID, Component>,

    /// We will support multiple workflows in one service with unique service-scoped IDs. For now, only one called "default".
    /// The workflows reference components by name (for now, always "default").
    pub workflows: BTreeMap<ID, Workflow>,

    pub status: ServiceStatus,
    pub testable: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ServiceStatus {
    Active,
    Stopped,
}

// we use UUID internally to store services, but the name is exposed on the management interface

#[derive(Error, Debug)]
pub enum OperatorError {
    // TODO: fill this with something better
    #[error("WASM code failed to compile")]
    InvalidWasmCode,

    #[error("Invalid ID: {0}")]
    ID(#[from] IDError),
}

// TODO: custom Deserialize that enforces validation rules
/// ID is meant to identify a component or a service (I don't think we need to enforce the distinction there, do we?)
/// It is a string, but with some strict validation rules. It must be lowecase alphanumeric: `[a-z0-9-_]{3,32}`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub struct ID(String);

impl ID {
    pub fn new(id: &str) -> Result<Self, IDError> {
        if id.len() < 3 || id.len() > 32 {
            return Err(IDError::LengthError);
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() && c.is_alphanumeric())
        {
            return Err(IDError::CharError);
        }
        Ok(Self(id.to_string()))
    }
}

impl AsRef<str> for ID {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Error, Debug)]
pub enum IDError {
    #[error("ID must be between 3 and 32 characters")]
    LengthError,
    #[error("ID must be lowercase alphanumeric")]
    CharError,
}

/***** Trigger subsystem *****/

pub struct TriggerManager {
    // TODO: implement this
}

impl TriggerManager {
    /// Create a new trigger manager.
    /// This returns the manager and a receiver for the trigger actions.
    /// Internally, all triggers may run in an async runtime and send results to the receiver.
    /// Externally, the operator can read the incoming tasks either sync or async
    pub fn create() -> (Self, mpsc::Receiver<TriggerAction>) {
        todo!();
    }

    pub fn add_trigger(&self, _trigger: TriggerData) -> Result<(), TriggerError> {
        todo!();
    }

    /// Remove one particular workflow
    pub fn remove_workflow(&self, _service_id: ID, _workflow_id: ID) -> Result<(), TriggerError> {
        todo!();
    }

    /// Remove all workflows for one service
    pub fn remove_service(&self, _service_id: ID) -> Result<(), TriggerError> {
        todo!();
    }

    /// List all registered triggers, by service ID
    pub fn list_triggers(&self, _service_id: ID) -> Result<Vec<TriggerData>, TriggerError> {
        todo!();
    }
}

/// Internal description of a registered trigger, to be indexed by associated IDs
pub struct TriggerData {
    pub service_id: ID,
    pub workflow_id: ID,
    pub trigger: Trigger,
}

/// The data returned from a trigger action
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TriggerAction {
    /// Identify which service and workflow this came from
    pub service_id: ID,
    pub workflow_id: ID,

    /// The data we got from the trigger
    pub result: TriggerResult,
}

/// This is the actual data we got from the trigger, used to feed into the component
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TriggerResult {
    Queue {
        /// The id from the task queue
        task_id: String,
        /// The input data associated with that task
        payload: Vec<u8>, // TODO: type with better serialization - Binary or serde_json::Value
    },
}

#[derive(Error, Debug)]
pub enum TriggerError {
    #[error("Cannot find service: {0}")]
    NoSuchService(ID),
    #[error("Cannot find workflow: {0} / {1}")]
    NoSuchWorkflow(ID, ID),
    #[error("Service exists, cannot register again: {0}")]
    ServiceAlreadyExists(ID),
    #[error("Workflow exists, cannot register again: {0} / {1}")]
    WorkflowAlreadyExists(ID, ID),
}

/***** Wasm Engine subsystem *****/

pub struct WasmEngine<S: CAStorage> {
    wasm_storage: S,
    // TODO: implement actual wasmtime engine here
}

// TODO: should we make some trait for quicker tasks where you just register closures for the digests?
impl<S: CAStorage> WasmEngine<S> {
    /// Create a new trigger manager.
    /// This returns the manager and a receiver for the trigger actions.
    /// Internally, all triggers may run in an async runtime and send results to the receiver.
    /// Externally, the operator can read the incoming tasks either sync or async
    pub fn new(wasm_storage: S) -> Self {
        Self { wasm_storage }
    }

    pub fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, WasmEngineError> {
        // TODO: validate bytecode is proper wasm with some wit interface
        let digest = self.wasm_storage.set_data(bytecode)?;
        Ok(digest)
    }

    // TODO: paginate this
    pub fn list_digests(&self) -> Result<Vec<Digest>, WasmEngineError> {
        // TODO: requires a range query on the castorage (.keys())
        todo!();
    }

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    pub fn execute_queue(
        &self,
        _digest: Digest,
        _request: Vec<u8>,
        _timestamp: u64,
    ) -> Result<Vec<u8>, WasmEngineError> {
        todo!();
    }
}

#[derive(Error, Debug)]
pub enum WasmEngineError {
    #[error("Storage: {0}")]
    Storage(#[from] CAStorageError),

    #[error("Invalid Wasm bytecode")]
    InvalidWasmCode,

    #[error("Wasm doesn't match expected wit interface")]
    WasmInterfaceMismatch,

    #[error("No wasm found for digest {0}")]
    UnknownDigest(Digest),
}
