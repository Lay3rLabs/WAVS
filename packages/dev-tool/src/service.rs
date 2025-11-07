use std::{collections::BTreeMap, sync::LazyLock};

use utils::filesystem::workspace_path;
use wavs_types::{
    AllowedHostPermission, Component, ComponentDigest, ComponentSource, Service, SignatureKind,
    Submit, WorkflowId,
};

pub static SERVICE_MANAGER: LazyLock<wavs_types::ServiceManager> =
    LazyLock::new(|| wavs_types::ServiceManager::Evm {
        chain: "evm:31337".parse().unwrap(),
        address: Default::default(),
    });

pub static WORKFLOW_ID: LazyLock<WorkflowId> =
    LazyLock::new(|| WorkflowId::new("workflow").unwrap());

pub static WAVS_COMPONENT_BYTES: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let wavs_component_path = workspace_path()
        .join("examples")
        .join("build")
        .join("components")
        .join("echo_data.wasm");
    std::fs::read(&wavs_component_path).expect("read echo_data.wasm")
});

pub static AGGREGATOR_COMPONENT_BYTES: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let aggregator_component_path = workspace_path()
        .join("examples")
        .join("build")
        .join("components")
        .join("simple_aggregator.wasm");
    std::fs::read(&aggregator_component_path).expect("read simple_aggregator.wasm")
});

pub fn create_service(sleep_ms: Option<u64>) -> Service {
    let wavs_component_digest = ComponentDigest::hash(&*WAVS_COMPONENT_BYTES);

    let aggregator_component_digest = ComponentDigest::hash(&*AGGREGATOR_COMPONENT_BYTES);

    Service {
        name: "Dev Test Service".to_string(),
        workflows: std::collections::BTreeMap::from([(
            WORKFLOW_ID.clone(),
            wavs_types::Workflow {
                trigger: wavs_types::Trigger::Manual,
                component: wavs_types::Component {
                    source: ComponentSource::Digest(wavs_component_digest),
                    permissions: wavs_types::Permissions {
                        file_system: false,
                        allowed_http_hosts: AllowedHostPermission::None,
                        raw_sockets: false,
                        dns_resolution: false,
                    },
                    fuel_limit: Some(u64::MAX),
                    time_limit_seconds: Some(100),
                    config: match sleep_ms {
                        Some(sleep) => BTreeMap::from([
                            ("sleep-kind".to_string(), "hotloop".to_string()),
                            ("sleep-ms".to_string(), sleep.to_string()),
                        ]),
                        None => BTreeMap::new(),
                    },
                    env_keys: std::collections::BTreeSet::new(),
                },
                // Use aggregator submit so the submission manager produces packets
                submit: Submit::Aggregator {
                    url: "http://127.0.0.1:12345".to_string(),
                    component: Box::new(Component {
                        source: ComponentSource::Digest(aggregator_component_digest),
                        permissions: wavs_types::Permissions {
                            file_system: false,
                            allowed_http_hosts: AllowedHostPermission::None,
                            raw_sockets: false,
                            dns_resolution: false,
                        },
                        fuel_limit: None,
                        time_limit_seconds: None,
                        config: BTreeMap::new(),
                        env_keys: std::collections::BTreeSet::new(),
                    }),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]),
        status: wavs_types::ServiceStatus::Active,
        manager: SERVICE_MANAGER.clone(),
    }
}
