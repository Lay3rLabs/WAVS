use std::collections::BTreeMap;
use wavs_types::{
    AllowedHostPermission, ComponentSource, Digest, Permissions, Service, ServiceID, Submit,
    Trigger, TriggerAction, TriggerConfig, TriggerData, Workflow, WorkflowID,
};

pub fn make_trigger_action(
    service: &Service,
    workflow_id: Option<WorkflowID>,
    input_data: Vec<u8>,
) -> TriggerAction {
    TriggerAction {
        config: TriggerConfig {
            service_id: service.id.clone(),
            workflow_id: workflow_id
                .unwrap_or_else(|| service.workflows.keys().next().cloned().unwrap()),
            trigger: service.workflows.values().next().unwrap().trigger.clone(),
        },
        data: TriggerData::Raw(input_data),
    }
}

pub fn make_service(wasm_digest: Digest) -> Service {
    let service_id = ServiceID::new("service-1").unwrap();
    let workflow_id = WorkflowID::new("workflow-1").unwrap();

    let workflow = Workflow {
        trigger: Trigger::Manual,
        component: wavs_types::Component {
            source: ComponentSource::Digest(wasm_digest),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            fuel_limit: None,
            time_limit_seconds: None,
            config: Default::default(),
            env_keys: Default::default(),
        },
        submit: Submit::None,
        aggregators: Vec::new(),
    };

    Service {
        id: service_id.clone(),
        name: "My Service".to_string(),
        workflows: BTreeMap::from([(workflow_id, workflow)]),
        status: wavs_types::ServiceStatus::Active,
        manager: wavs_types::ServiceManager::Evm {
            chain_name: "noop".parse().unwrap(),
            address: Default::default(),
        },
    }
}
