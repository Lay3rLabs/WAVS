// This is currently a scratchpad to define some interfaces for the system level.
// It probably should be pulled into multiple files before merging, but I think easier to visualize and review all together first.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::Digest;

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
    pub component: String,
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
        poll_interval: u64,
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
    // Public identifier
    pub name: String,
    // Internal identifier, for safe internal identifier (may be different on each node).
    // This can be used to construct filesystem paths, without opening up to path traversal attacks.
    pub uuid: Uuid,
    /// We will supoort multiple names components in one service. For now, just add one called "default".
    /// This allows clean mapping from backwards-compatible API endpoints.
    pub components: BTreeMap<String, Component>,

    /// We will support multiple workflows in one service. For now, only one.
    /// These probably don't need to be named (externally), but we can add an identifier if it makes it easier to track.
    /// The workflows reference components by name (for now, always "default").
    pub workflows: Vec<Workflow>,

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
}
