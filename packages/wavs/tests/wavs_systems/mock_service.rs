use utils::{storage::db::WavsDb, test_utils::address::rand_address_evm};
use wavs::services::Services;
use wavs_types::{
    Component, ComponentDigest, ComponentSource, Service, ServiceManager, SignatureKind, Submit,
    Trigger, Workflow,
};

pub fn mock_services() -> Services {
    wavs::services::Services::new(WavsDb::new().unwrap())
}

pub fn mock_service() -> Service {
    wavs_types::Service {
        name: "serv1".to_string(),
        status: wavs_types::ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:anvil".parse().unwrap(),
            address: rand_address_evm(),
        },
        workflows: vec![(
            "workflow-1".parse().unwrap(),
            Workflow {
                trigger: Trigger::Manual,
                component: Component::new(ComponentSource::Digest(ComponentDigest::hash([0; 32]))),
                submit: Submit::Aggregator {
                    component: Box::new(Component::new(ComponentSource::Digest(
                        ComponentDigest::hash([0; 32]),
                    ))),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]
        .into_iter()
        .collect(),
    }
}
