use std::{collections::BTreeMap, ops::Bound};

use layer_climb::prelude::Address;
use serde::{Deserialize, Serialize};

use super::{submission::ChainMessage, trigger::TriggerAction, Trigger, ID};
use crate::{AppContext, Digest};

/// This is the highest-level container for the system.
/// The http server can hold this in state and interact with the "management interface".
/// The other components route to each other via this one.
///
/// It uses internal mutability pattern, so we can have multiple references to it.
/// It should implement Send and Sync so it can be used in async code.
///
/// These types should not be raw from the user, but parsed from the JSON structs, validated,
/// and converted into our internal structs
pub trait DispatchManager: Send + Sync {
    type Error;

    fn start(&self, ctx: AppContext) -> Result<(), Self::Error>;

    fn run_trigger(&self, action: TriggerAction) -> Result<Option<ChainMessage>, Self::Error>;

    /// Used to install new wasm bytecode into the system.
    /// Either the bytecode is provided directly, or it is downloaded from a URL.
    fn store_component(&self, source: WasmSource) -> Result<Digest, Self::Error>;

    fn add_service(&self, service: Service) -> Result<(), Self::Error>;

    fn remove_service(&self, id: ID) -> Result<(), Self::Error>;

    fn list_services(
        &self,
        bounds_start: Bound<&str>,
        bounds_end: Bound<&str>,
    ) -> Result<Vec<Service>, Self::Error>;

    /// TODO: pagination
    fn list_component_digests(&self) -> Result<Vec<Digest>, Self::Error>;

    // TODO: this would be nicer so we can just pass in a range
    // but then we run into problems with storing DispatchManager as a trait object
    // fn list_services<'a>(&self, bounds: impl RangeBounds<&'a str>) -> Result<Vec<Service>, Self::Error>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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

// FIXME: happy for a better name.
/// This captures the triggers we listen to, the components we run, and how we submit the result
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub trigger: Trigger,
    /// A reference to which component to run with this data - for now, always "default"
    pub component: ID,

    /// How to submit the result of the component.
    /// May be unset for eg cron jobs that just update internal state and don't submit anything
    pub submit: Option<Submit>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Submit {
    /// Writing a transaction directly to the layer verifier contract
    LayerVerifierTx {
        /// The hd index of the mnemonic to sign with
        hd_index: u32,
        // The address of the verifier contract to submit to
        // Note: To keep the same axum API, the http server can query this from the task queue contract (which is provided)
        // I want to break these hard dependencies internally, so Dispatcher doesn't assume those connections between contracts
        verifier_addr: Address,
    }, // Example alternative is making a message and BLS signing it, then submitting to an aggregator
    /// Sending a message to the aggregator on eth chain
    EthAggregatorTx {},
    /// Sending a message to the aggregator on eth chain
    EthSignedMessage {
        /// The hd index of the mnemonic to sign with
        hd_index: u32,
    },
}

impl Submit {
    pub fn layer_verifier_tx(hd_index: u32, verifier_addr: Address) -> Self {
        Submit::LayerVerifierTx {
            hd_index,
            verifier_addr,
        }
    }
    pub fn eth_aggregator_tx() -> Self {
        Submit::EthAggregatorTx {}
    }
    pub fn eth_signed_message(hd_index: u32) -> Self {
        Submit::EthSignedMessage { hd_index }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Copy)]
#[serde(rename_all = "camelCase")]
pub enum ServiceStatus {
    Active,
    // we could have more like Stopped, Failed, Cooldown, etc.
    // Technically these exist in 0.2, but only on response, and we never actually respond with them for now
    // so it doesn't break backwards compat to remove them:
    // Failed,
    // MissingWasm,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub wasm: Digest,
    // What permissions this component has.
    // These are currently not enforced, you can pass in Default::default() for now
    pub permissions: Permissions,
    pub env: Vec<(String, String)>,
}

impl Component {
    pub fn new(digest: &Digest) -> Self {
        Self {
            wasm: digest.clone(),
            permissions: Default::default(),
            env: vec![],
        }
    }
}

// TODO: we can remove / change defaults in 0.3.0, they are needed for 0.2.0 compat
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Permissions {
    /// If it can talk to http hosts on the network
    pub allowed_http_hosts: AllowedHostPermission,
    /// If it can write to it's own local directory in the filesystem
    pub file_system: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            allowed_http_hosts: AllowedHostPermission::default(),
            file_system: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AllowedHostPermission {
    #[default] // only for 0.2.0
    All,
    Only(Vec<String>),
    // #[default] // this is for 0.3.0
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backwards_compat_permission_json() {
        let json = "{}";
        let permissions: Permissions = serde_json::from_str(json).unwrap();
        assert_eq!(permissions.allowed_http_hosts, AllowedHostPermission::All);
        assert!(permissions.file_system);

        let json = r#"{"allowedHttpHosts":"none","fileSystem":false}"#;
        let permissions: super::Permissions = serde_json::from_str(json).unwrap();
        assert_eq!(permissions.allowed_http_hosts, AllowedHostPermission::None,);
        assert!(!permissions.file_system);
    }

    #[test]
    fn permission_defaults() {
        let permissions_json: Permissions = serde_json::from_str("{}").unwrap();
        let permissions_default: Permissions = Permissions::default();

        assert_eq!(permissions_json, permissions_default);
        assert_eq!(
            permissions_default.allowed_http_hosts,
            AllowedHostPermission::All
        );
        assert!(permissions_default.file_system);
    }
}
