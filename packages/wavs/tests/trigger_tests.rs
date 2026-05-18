#![cfg(feature = "dev")]
use std::num::NonZero;

use wavs::{config::Config, dispatcher::DispatcherCommand, subsystems::trigger::TriggerManager};
use wavs_types::{
    ChainKey, Component, ComponentDigest, ComponentSource, Service, ServiceId, ServiceManager,
    ServiceStatus, SignatureKind, Submit, Timestamp, Trigger, TriggerConfig, Workflow, WorkflowId,
};

use layer_climb::prelude::*;
use utils::{
    storage::db::WavsDb,
    telemetry::TriggerMetrics,
    test_utils::address::{rand_address_evm, rand_event_evm},
};

#[test]
fn core_trigger_lookups() {
    let config = Config::default();

    let services = wavs::services::Services::new(WavsDb::new().unwrap());

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
            Trigger::CosmosContractEvent { address, .. } => address.clone().into(),
            _ => panic!("unexpected trigger type"),
        }
    }
}

#[tokio::test]
async fn block_interval_trigger_is_removed_when_config_is_gone() {
    let config = Config::default();

    let services = wavs::services::Services::new(WavsDb::new().unwrap());

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

    let services = wavs::services::Services::new(WavsDb::new().unwrap());
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

#[test]
fn solana_program_event_lookup() {
    use wavs_types::{SolanaAddress, SolanaCommitment, SolanaEventFilter};

    let config = Config::default();
    let services = wavs::services::Services::new(WavsDb::new().unwrap());
    let (trigger_to_dispatcher_tx, _) = crossbeam::channel::unbounded::<DispatcherCommand>();
    let manager = TriggerManager::new(
        &config,
        TriggerMetrics::new(opentelemetry::global::meter("trigger-test-metrics")),
        services,
        trigger_to_dispatcher_tx,
    )
    .unwrap();

    let chain = ChainKey::new("solana:devnet").unwrap();
    let system_program = SolanaAddress::from_base58("11111111111111111111111111111111").unwrap();
    let token_program =
        SolanaAddress::from_base58("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();

    // Two triggers for the same (chain, program_id) but with different
    // filters: an Anchor discriminator and a log-substring. The lookup
    // table should group both under (chain, system_program); per-filter
    // dispatch is the dispatcher's job.
    let service_a = ServiceId::hash("svc-a");
    let workflow_a = WorkflowId::new("wf-a").unwrap();
    let trigger_a = TriggerConfig::solana_program_event(
        service_a.clone(),
        workflow_a.to_string().as_str(),
        chain.to_string().as_str(),
        system_program,
        SolanaEventFilter::Discriminator(vec![1, 2, 3, 4, 5, 6, 7, 8]),
        SolanaCommitment::Confirmed,
    );

    let service_b = ServiceId::hash("svc-b");
    let workflow_b = WorkflowId::new("wf-b").unwrap();
    let trigger_b = TriggerConfig::solana_program_event(
        service_b.clone(),
        workflow_b.to_string().as_str(),
        chain.to_string().as_str(),
        system_program,
        SolanaEventFilter::LogContains("matched".to_string()),
        SolanaCommitment::Confirmed,
    );

    // Third trigger on a different program — should not collide with the
    // first two.
    let service_c = ServiceId::hash("svc-c");
    let workflow_c = WorkflowId::new("wf-c").unwrap();
    let trigger_c = TriggerConfig::solana_program_event(
        service_c.clone(),
        workflow_c.to_string().as_str(),
        chain.to_string().as_str(),
        token_program,
        SolanaEventFilter::LogContains("transfer".to_string()),
        SolanaCommitment::Confirmed,
    );

    manager.get_lookup_maps().add_trigger(trigger_a).unwrap();
    manager.get_lookup_maps().add_trigger(trigger_b).unwrap();
    manager.get_lookup_maps().add_trigger(trigger_c).unwrap();

    {
        let lock = manager
            .get_lookup_maps()
            .triggers_by_solana_program
            .read()
            .unwrap();
        // (chain, system_program) bucket has both triggers
        let system_bucket = lock.get(&(chain.clone(), system_program)).unwrap();
        assert_eq!(system_bucket.len(), 2);
        // (chain, token_program) bucket has the third
        let token_bucket = lock.get(&(chain.clone(), token_program)).unwrap();
        assert_eq!(token_bucket.len(), 1);
    }

    // Removing workflow A should leave B and C; the (chain,
    // system_program) bucket should still contain one entry.
    manager
        .get_lookup_maps()
        .remove_workflow(service_a, workflow_a)
        .unwrap();
    {
        let lock = manager
            .get_lookup_maps()
            .triggers_by_solana_program
            .read()
            .unwrap();
        let system_bucket = lock.get(&(chain.clone(), system_program)).unwrap();
        assert_eq!(system_bucket.len(), 1);
    }

    // Removing service B should empty the (chain, system_program)
    // bucket entirely (the lookup table garbage-collects empty sets).
    manager.remove_service(service_b).unwrap();
    {
        let lock = manager
            .get_lookup_maps()
            .triggers_by_solana_program
            .read()
            .unwrap();
        assert!(lock.get(&(chain.clone(), system_program)).is_none());
        // Token bucket is untouched.
        let token_bucket = lock.get(&(chain.clone(), token_program)).unwrap();
        assert_eq!(token_bucket.len(), 1);
    }
}

/// Replay-protection regression test for the slice 3 deliverable.
///
/// The SVM design doc requires that the same
/// `(slot, signature, instruction_index, inner_instruction_index, log_index)`
/// tuple does NOT re-fire the operator, including across deliberate
/// `solana-pubsub` reconnects. This test simulates the reconnect by
/// pushing the same `SolanaStreamLog` through the dispatcher twice:
/// the first call should produce a `DispatcherCommand::Trigger`; the
/// second call (mimicking a reconnect that replays the same notification)
/// should be silently dropped by the dedup cache.
#[test]
fn solana_program_event_replay_protection() {
    use wavs::subsystems::trigger::streams::solana_stream::SolanaStreamLog;
    use wavs_types::{SolanaAddress, SolanaCommitment, SolanaEventFilter};

    let config = Config::default();
    let services = wavs::services::Services::new(WavsDb::new().unwrap());
    let (trigger_to_dispatcher_tx, _) = crossbeam::channel::unbounded::<DispatcherCommand>();
    let manager = TriggerManager::new(
        &config,
        TriggerMetrics::new(opentelemetry::global::meter("trigger-test-metrics")),
        services,
        trigger_to_dispatcher_tx,
    )
    .unwrap();

    // Activate the test service so the lookup table will surface it.
    let chain = ChainKey::new("solana:devnet").unwrap();
    let program = SolanaAddress::from_base58("11111111111111111111111111111111").unwrap();

    let workflow_id = WorkflowId::new("wf-replay").unwrap();

    let trigger = wavs_types::Trigger::SolanaProgramEvent {
        chain: chain.clone(),
        program_id: program,
        filter: SolanaEventFilter::LogContains("Program log: matched".to_string()),
        commitment: SolanaCommitment::Confirmed,
    };
    let workflow = Workflow {
        component: Component {
            source: ComponentSource::Digest(ComponentDigest::hash([0u8; 32])),
            permissions: Default::default(),
            fuel_limit: None,
            time_limit_seconds: None,
            config: Default::default(),
            env_keys: Default::default(),
        },
        trigger: trigger.clone(),
        submit: Submit::None,
    };
    let mut workflows = std::collections::BTreeMap::new();
    workflows.insert(workflow_id.clone(), workflow);
    let manager_service = Service {
        name: "replay-test".to_string(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: ChainKey::new("evm:anvil").unwrap(),
            address: rand_address_evm(),
        },
        workflows,
    };
    let service_id = manager_service.id();
    manager.services.save(&manager_service).unwrap();
    manager
        .get_lookup_maps()
        .add_trigger(wavs_types::TriggerConfig {
            service_id: service_id.clone(),
            workflow_id: workflow_id.clone(),
            trigger,
        })
        .unwrap();

    // The replay-identity tuple is identical between the two pushes.
    let log = SolanaStreamLog {
        slot: 42,
        signature: "5".repeat(88), // base58-shaped placeholder
        instruction_index: 0,
        inner_instruction_index: None,
        log_index: 3,
        program_id: program,
        raw_log: "Program log: matched the discriminator".to_string(),
    };

    // First observation: a single Trigger command should land.
    let first = manager.handle_solana_logs(chain.clone(), 42, vec![log.clone()]);
    assert_eq!(
        first.len(),
        1,
        "first observation should produce exactly one DispatcherCommand::Trigger; got {first:?}"
    );

    // Reconnect simulation: the exact same notification arrives again.
    // The replay cache must drop it.
    let second = manager.handle_solana_logs(chain.clone(), 42, vec![log.clone()]);
    assert!(
        second.is_empty(),
        "second observation of identical replay-identity should be deduped; got {second:?}"
    );

    // A different log_index in the same transaction is a genuinely new
    // event and must NOT be deduped.
    let next_log = SolanaStreamLog {
        log_index: 4,
        ..log.clone()
    };
    let next = manager.handle_solana_logs(chain.clone(), 42, vec![next_log]);
    assert_eq!(
        next.len(),
        1,
        "distinct log_index must produce a new Trigger; got {next:?}"
    );

    // And a different signature with otherwise-identical fields is also
    // a new event.
    let other_tx = SolanaStreamLog {
        signature: "6".repeat(88),
        ..log.clone()
    };
    let other = manager.handle_solana_logs(chain.clone(), 42, vec![other_tx]);
    assert_eq!(other.len(), 1, "distinct signature must produce a new Trigger");
}
