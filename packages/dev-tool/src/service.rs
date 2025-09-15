use std::sync::LazyLock;

use utils::filesystem::workspace_path;
use wavs_types::{
    AllowedHostPermission, Component, ComponentDigest, ComponentSource, Service, SignatureKind,
    Submit, WorkflowId,
};

pub static SERVICE: LazyLock<Service> = LazyLock::new(|| {
    let workflow_id = WorkflowId::new("dev-trigger-workflow".to_string()).unwrap();

    let wavs_component_digest = ComponentDigest::hash(&*WAVS_COMPONENT_BYTES);

    let aggregator_component_digest = ComponentDigest::hash(&*AGGREGATOR_COMPONENT_BYTES);

    Service {
        name: "Dev Test Service".to_string(),
        workflows: std::collections::BTreeMap::from([(
            workflow_id.clone(),
            wavs_types::Workflow {
                trigger: wavs_types::Trigger::Manual,
                component: wavs_types::Component {
                    source: ComponentSource::Digest(wavs_component_digest),
                    permissions: wavs_types::Permissions {
                        file_system: false,
                        allowed_http_hosts: AllowedHostPermission::None,
                    },
                    fuel_limit: None,
                    time_limit_seconds: None,
                    config: std::collections::BTreeMap::new(),
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
                        },
                        fuel_limit: None,
                        time_limit_seconds: None,
                        config: std::collections::BTreeMap::new(),
                        env_keys: std::collections::BTreeSet::new(),
                    }),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]),
        status: wavs_types::ServiceStatus::Active,
        manager: wavs_types::ServiceManager::Evm {
            chain: "evm:31337".parse().unwrap(),
            address: Default::default(),
        },
    }
});

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
