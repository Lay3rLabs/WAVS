use std::collections::BTreeMap;
use wavs_types::{
    AllowedHostPermission, ComponentDigest, ComponentSource, Permissions, Service, SignatureKind,
    Submit, Trigger, TriggerAction, TriggerConfig, TriggerData, Workflow, WorkflowId,
};

#[allow(dead_code)]
pub fn make_trigger_action(
    service: &Service,
    workflow_id: Option<WorkflowId>,
    input_data: Vec<u8>,
) -> TriggerAction {
    TriggerAction {
        config: TriggerConfig {
            service_id: service.id().clone(),
            workflow_id: workflow_id
                .unwrap_or_else(|| service.workflows.keys().next().cloned().unwrap()),
            trigger: service.workflows.values().next().unwrap().trigger.clone(),
        },
        data: TriggerData::Raw(input_data),
    }
}

pub fn make_service(wasm_digest: ComponentDigest, config: BTreeMap<String, String>) -> Service {
    let workflow_id = WorkflowId::new("workflow-1").unwrap();
    let component = wavs_types::Component {
        source: ComponentSource::Digest(wasm_digest),
        permissions: Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
            raw_sockets: true,
            dns_resolution: true,
        },
        fuel_limit: None,
        time_limit_seconds: None,
        config,
        env_keys: Default::default(),
    };
    let workflow = Workflow {
        trigger: Trigger::Manual,
        component: component.clone(),
        submit: Submit::Aggregator {
            component: Box::new(component),
            signature_kind: SignatureKind::evm_default(),
        },
    };

    Service {
        name: "My Service".to_string(),
        workflows: BTreeMap::from([(workflow_id, workflow)]),
        status: wavs_types::ServiceStatus::Active,
        manager: wavs_types::ServiceManager::Evm {
            chain: "evm:noop".parse().unwrap(),
            address: Default::default(),
        },
    }
}
