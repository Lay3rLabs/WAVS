#![cfg(feature = "dev")]
use std::num::NonZero;

use wavs::{config::Config, dispatcher::DispatcherCommand, subsystems::trigger::TriggerManager};
use wavs_types::{
    ChainKey, Component, ComponentDigest, ComponentSource, Service, ServiceId, ServiceManager,
    ServiceStatus, SignatureKind, Submit, Timestamp, Trigger, TriggerConfig, Workflow, WorkflowId,
};

use layer_climb::prelude::*;
use utils::{
    storage::db::RedbStorage,
    telemetry::TriggerMetrics,
    test_utils::address::{rand_address_evm, rand_event_evm},
};

#[test]
fn core_trigger_lookups() {
    let config = Config::default();

    let data_dir = tempfile::tempdir().unwrap();
    let services =
        wavs::services::Services::new(RedbStorage::new(data_dir.path().join("db")).unwrap());

    let (trigger_to_dispatcher_tx, _) = crossbeam::channel::unbounded::<DispatcherCommand>();
    let manager = TriggerManager::new(
        &config,
        TriggerMetrics::new(opentelemetry::global::meter("trigger-test-metrics")),
        services,
        trigger_to_dispatcher_tx,
    )
    .unwrap();

    let service_id_1 = ServiceId::hash("service-1");
    let workflow_id_1 = WorkflowId::new("workflow-1").unwrap();

    let service_id_2 = ServiceId::hash("service-2");
    let workflow_id_2 = WorkflowId::new("workflow-2").unwrap();

    let task_queue_addr_1_1 = rand_address_evm();
    let task_queue_addr_1_2 = rand_address_evm();
    let task_queue_addr_2_1 = rand_address_evm();
    let task_queue_addr_2_2 = rand_address_evm();

    let trigger_1_1 = TriggerConfig::evm_contract_event(
        service_id_1.clone(),
        workflow_id_1.to_string().as_str(),
        task_queue_addr_1_1,
        "evm:anvil",
        rand_event_evm(),
    );
    let trigger_1_2 = TriggerConfig::evm_contract_event(
        service_id_1.clone(),
        workflow_id_2.to_string().as_str(),
        task_queue_addr_1_2,
        "evm:anvil",
        rand_event_evm(),
    );
    let trigger_2_1 = TriggerConfig::evm_contract_event(
        service_id_2.clone(),
        workflow_id_1.to_string().as_str(),
        task_queue_addr_2_1,
        "evm:anvil",
        rand_event_evm(),
    );
    let trigger_2_2 = TriggerConfig::evm_contract_event(
        service_id_2.clone(),
        workflow_id_2.to_string().as_str(),
        task_queue_addr_2_2,
        "evm:anvil",
        rand_event_evm(),
    );

    manager.get_lookup_maps().add_trigger(trigger_1_1).unwrap();
    manager.get_lookup_maps().add_trigger(trigger_1_2).unwrap();
    manager.get_lookup_maps().add_trigger(trigger_2_1).unwrap();
    manager.get_lookup_maps().add_trigger(trigger_2_2).unwrap();

    let triggers_service_1 = manager
        .get_lookup_maps()
        .configs_for_service(service_id_1.clone())
        .unwrap();

    assert_eq!(triggers_service_1.len(), 2);
    assert_eq!(triggers_service_1[0].service_id, service_id_1);
    assert_eq!(triggers_service_1[0].workflow_id, workflow_id_1);
    assert_eq!(
        get_trigger_addr(&triggers_service_1[0].trigger),
        task_queue_addr_1_1.into()
    );
    assert_eq!(triggers_service_1[1].service_id, service_id_1);
    assert_eq!(triggers_service_1[1].workflow_id, workflow_id_2);
    assert_eq!(
        get_trigger_addr(&triggers_service_1[1].trigger),
        task_queue_addr_1_2.into()
    );

    let triggers_service_2 = manager
        .get_lookup_maps()
        .configs_for_service(service_id_2.clone())
        .unwrap();

    assert_eq!(triggers_service_2.len(), 2);
    assert_eq!(triggers_service_2[0].service_id, service_id_2);
    assert_eq!(triggers_service_2[0].workflow_id, workflow_id_1);
    assert_eq!(
        get_trigger_addr(&triggers_service_2[0].trigger),
        task_queue_addr_2_1.into()
    );
    assert_eq!(triggers_service_2[1].service_id, service_id_2);
    assert_eq!(triggers_service_2[1].workflow_id, workflow_id_2);
    assert_eq!(
        get_trigger_addr(&triggers_service_2[1].trigger),
        task_queue_addr_2_2.into()
    );

    manager
        .get_lookup_maps()
        .remove_workflow(service_id_1.clone(), workflow_id_1)
        .unwrap();
    let triggers_service_1 = manager
        .get_lookup_maps()
        .configs_for_service(service_id_1.clone())
        .unwrap();
    let triggers_service_2 = manager
        .get_lookup_maps()
        .configs_for_service(service_id_2.clone())
        .unwrap();
    assert_eq!(triggers_service_1.len(), 1);
    assert_eq!(triggers_service_2.len(), 2);

    manager.remove_service(service_id_2.clone()).unwrap();
    let triggers_service_1 = manager
        .get_lookup_maps()
        .configs_for_service(service_id_1.clone())
        .unwrap();
    let _triggers_service_2_err = manager
        .get_lookup_maps()
        .configs_for_service(service_id_2.clone())
        .unwrap_err();
    assert_eq!(triggers_service_1.len(), 1);

    fn get_trigger_addr(trigger: &Trigger) -> Address {
        match trigger {
            Trigger::EvmContractEvent { address, .. } => (*address).into(),
            Trigger::CosmosContractEvent { address, .. } => address.clone(),
            _ => panic!("unexpected trigger type"),
        }
    }
}

#[tokio::test]
async fn block_interval_trigger_is_removed_when_config_is_gone() {
    let config = Config::default();

    let data_dir = tempfile::tempdir().unwrap();
    let services =
        wavs::services::Services::new(RedbStorage::new(data_dir.path().join("db")).unwrap());

    let (trigger_to_dispatcher_tx, _) = crossbeam::channel::unbounded::<DispatcherCommand>();
    let manager = TriggerManager::new(
        &config,
        TriggerMetrics::new(opentelemetry::global::meter("trigger-test-metrics")),
        services.clone(),
        trigger_to_dispatcher_tx,
    )
    .unwrap();

    let workflow_id = WorkflowId::new("workflow-1").unwrap();
    let chain = ChainKey::new("evm:local").unwrap();

    // set number of blocks to 1 to fire the trigger immediately
    let n_blocks = NonZero::new(1).unwrap();

    let service = Service {
        name: "Big Square AVS".to_string(),
        workflows: [(
            workflow_id.clone(),
            Workflow {
                component: Component::new(ComponentSource::Digest(ComponentDigest::hash([0; 32]))),
                trigger: Trigger::BlockInterval {
                    chain: chain.clone(),
                    n_blocks,
                    start_block: None,
                    end_block: None,
                },
                submit: Submit::Aggregator {
                    url: "http://example.com/aggregator".to_string(),
                    component: Box::new(Component::new(ComponentSource::Digest(
                        ComponentDigest::hash([1, 2, 3]),
                    ))),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]
        .into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: chain.clone(),
            address: rand_address_evm(),
        },
    };
    services.save(&service).unwrap();

    let trigger = TriggerConfig::block_interval_event(
        service.id(),
        workflow_id.to_string().as_str(),
        chain.to_string().as_str(),
        n_blocks,
    );

    manager
        .get_lookup_maps()
        .add_trigger(trigger.clone())
        .unwrap();

    let service_2 = Service {
        manager: ServiceManager::Evm {
            chain: chain.clone(),
            address: rand_address_evm(),
        },
        ..service.clone()
    };

    let trigger = TriggerConfig::block_interval_event(
        service_2.id(),
        workflow_id.to_string().as_str(),
        chain.to_string().as_str(),
        n_blocks,
    );
    manager
        .get_lookup_maps()
        .add_trigger(trigger.clone())
        .unwrap();

    services.save(&service_2).unwrap();

    // Verify we have two scheduled triggers
    assert_eq!(
        manager
            .get_lookup_maps()
            .block_schedulers
            .get(&chain)
            .unwrap()
            .len(),
        2
    );

    // Remove one trigger and verify we have one left
    manager
        .get_lookup_maps()
        .remove_workflow(service.id(), workflow_id.clone())
        .unwrap();

    let trigger_actions = manager.process_blocks(chain.clone(), 10);

    // verify only one trigger action is generated
    assert_eq!(trigger_actions.len(), 1);
    assert_eq!(
        manager
            .get_lookup_maps()
            .block_schedulers
            .get(&chain)
            .unwrap()
            .len(),
        1
    );

    // remove the last trigger config
    manager
        .get_lookup_maps()
        .remove_workflow(service_2.id(), workflow_id.clone())
        .unwrap();

    let trigger_actions = manager.process_blocks(chain.clone(), 20);

    // verify no trigger action is generated this time
    assert!(trigger_actions.is_empty());
    assert_eq!(
        manager
            .get_lookup_maps()
            .block_schedulers
            .get(&chain)
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn cron_trigger_is_removed_when_config_is_gone() {
    // Setup configuration and manager
    let config = Config::default();

    let data_dir = tempfile::tempdir().unwrap();
    let services =
        wavs::services::Services::new(RedbStorage::new(data_dir.path().join("db")).unwrap());
    let (trigger_to_dispatcher_tx, _) = crossbeam::channel::unbounded::<DispatcherCommand>();
    let manager = TriggerManager::new(
        &config,
        TriggerMetrics::new(opentelemetry::global::meter("trigger-test-metrics")),
        services,
        trigger_to_dispatcher_tx,
    )
    .unwrap();

    // Create service and workflow IDs
    let service_id = ServiceId::hash("service-1");
    let workflow_id = WorkflowId::new("workflow-1").unwrap();

    // Set up the first trigger
    let trigger1 = TriggerConfig {
        service_id: service_id.clone(),
        workflow_id: workflow_id.clone(),
        trigger: Trigger::Cron {
            schedule: "* * * * * *".to_owned(),
            start_time: None,
            end_time: None,
        },
    };
    manager.get_lookup_maps().add_trigger(trigger1).unwrap();

    // Set up the second trigger
    let service_id2 = ServiceId::hash("service-2");
    let trigger2 = TriggerConfig {
        service_id: service_id2.clone(),
        workflow_id: workflow_id.clone(),
        trigger: Trigger::Cron {
            schedule: "* * * * * *".to_owned(),
            start_time: None,
            end_time: None,
        },
    };
    manager.get_lookup_maps().add_trigger(trigger2).unwrap();

    // first tick is now
    let lookup_ids = manager
        .get_lookup_maps()
        .cron_scheduler
        .lock()
        .unwrap()
        .tick(Timestamp::from_datetime(chrono::Utc::now()).unwrap());
    assert_eq!(
        lookup_ids.len(),
        0,
        "Expected first tick to have no triggers"
    );

    // Use a future time to process triggers
    let future_time =
        Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::seconds(10)).unwrap();
    let lookup_ids = manager
        .get_lookup_maps()
        .cron_scheduler
        .lock()
        .unwrap()
        .tick(future_time);

    // Verify both triggers fire
    assert_eq!(lookup_ids.len(), 2, "Expected 2 triggers to fire");

    // Remove the first trigger
    manager
        .get_lookup_maps()
        .remove_workflow(service_id.clone(), workflow_id.clone())
        .unwrap();

    // Process triggers again
    let future_time =
        Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::seconds(10)).unwrap();
    let lookup_ids = manager
        .get_lookup_maps()
        .cron_scheduler
        .lock()
        .unwrap()
        .tick(future_time);

    // Verify only one trigger fires now
    assert_eq!(
        lookup_ids.len(),
        1,
        "Expected 1 trigger to fire after removing one"
    );

    // Remove the second trigger
    manager
        .get_lookup_maps()
        .remove_workflow(service_id2.clone(), workflow_id.clone())
        .unwrap();

    // Process triggers one more time
    let future_time =
        Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::seconds(10)).unwrap();
    let lookup_ids = manager
        .get_lookup_maps()
        .cron_scheduler
        .lock()
        .unwrap()
        .tick(future_time);

    // Verify no triggers fire
    assert!(
        lookup_ids.is_empty(),
        "Expected no triggers to fire after removing all"
    );
}
