// these are like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected

use alloy_primitives::LogData;
use example_types::SquareRequest;
use utils::{
    context::AppContext,
    test_utils::{
        address::{rand_address_cosmos, rand_address_evm},
        mock_engine::{COMPONENT_ECHO_DATA_BYTES, COMPONENT_SQUARE_BYTES},
    },
};
mod wavs_systems;
use alloy_sol_types::{sol, SolValue};
use wavs::dispatcher::DispatcherCommand;
use wavs_systems::{mock_app::MockE2ETestRunner, mock_submissions::wait_for_submission_messages};
use wavs_types::{
    ChainKey, Component, ComponentSource, EventId, Service, ServiceManager, SignatureKind, Submit,
    Trigger, TriggerAction, TriggerConfig, TriggerData, Workflow, WorkflowId,
};

// Solidity types used by mock EVM event encoding
sol! {
    struct TriggerInfo { uint64 triggerId; address creator; bytes data; }
}

#[test]
fn mock_e2e_trigger_flow() {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let task_queue_address = rand_address_cosmos();

    // block and wait for creating the service
    let service_id = runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            let digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_SQUARE_BYTES)
                .unwrap();
            runner
                .create_service(None, ComponentSource::Digest(digest))
                .await
        }
    });

    // now pretend like we're reading some triggers off the chain
    runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            runner
                .send_trigger(
                    service_id.clone(),
                    WorkflowId::default().to_string().as_str(),
                    &task_queue_address.clone(),
                    &SquareRequest { x: 3 },
                    "evm:anvil",
                )
                .await;
            runner
                .send_trigger(
                    service_id,
                    WorkflowId::default().to_string().as_str(),
                    &task_queue_address,
                    &SquareRequest { x: 21 },
                    "evm:anvil",
                )
                .await;
        }
    });

    // block and wait for triggers to go through the whole flow
    wait_for_submission_messages(&runner.dispatcher.submission_manager, 2, None).unwrap();

    // elsewhere we know that the component is executing, no need to check the actual results here
    // since Submit is None
}

#[test]
fn mock_e2e_service_lifecycle() {
    let runner = MockE2ETestRunner::new(AppContext::new());
    // block and wait for creating the service

    runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            let resp = runner.list_services().await;

            assert!(resp.services.is_empty());
            assert!(resp.component_digests.is_empty());

            // add services in order
            let digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_SQUARE_BYTES)
                .unwrap();

            let mut service_ids = Vec::new();
            for i in 1..=3 {
                service_ids.push(
                    runner
                        .create_service(
                            Some(format!("service-{i}")),
                            ComponentSource::Digest(digest.clone()),
                        )
                        .await,
                );
            }

            let mut resp = runner.list_services().await;
            resp.services.sort_by_key(|a| a.id());
            service_ids.sort();

            assert_eq!(resp.services.len(), 3);
            assert_eq!(resp.component_digests.len(), 1);
            assert_eq!(resp.services[0].id(), service_ids[0]);
            assert_eq!(resp.services[1].id(), service_ids[1]);
            assert_eq!(resp.services[2].id(), service_ids[2]);

            let service_managers: Vec<ServiceManager> =
                resp.services.iter().map(|s| s.manager.clone()).collect();

            // add an orphaned digest
            let _orphaned_digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_ECHO_DATA_BYTES)
                .unwrap();

            let resp = runner.list_services().await;
            assert_eq!(resp.services.len(), 3);
            assert_eq!(resp.component_digests.len(), 2);

            // selectively delete services 1 and 3, leaving just 2

            runner
                .delete_services(vec![
                    service_managers[0].clone(),
                    service_managers[2].clone(),
                ])
                .await;

            let resp = runner.list_services().await;

            assert_eq!(resp.services.len(), 1);
            assert_eq!(resp.component_digests.len(), 2);
            assert_eq!(resp.services[0].id(), service_ids[1]);

            // and make sure we can delete the last one but still get an empty list
            runner
                .delete_services(vec![service_managers[1].clone()])
                .await;

            let resp = runner.list_services().await;

            assert!(resp.services.is_empty());
            assert_eq!(resp.component_digests.len(), 2);
        }
    });
}

#[test]
fn mock_e2e_component_none() {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let task_queue_address = rand_address_evm();

    // block and wait for creating the service
    let service_id = runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            let digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_SQUARE_BYTES)
                .unwrap();

            runner
                .create_service(None, ComponentSource::Digest(digest))
                .await
        }
    });

    // now pretend like we're reading some triggers off the chain
    runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            runner
                .send_trigger(
                    service_id,
                    WorkflowId::default().to_string().as_str(),
                    &task_queue_address.into(),
                    &SquareRequest { x: 3 },
                    "evm:anvil",
                )
                .await;
        }
    });

    // this _should_ error because submission is not fired
    wait_for_submission_messages(&runner.dispatcher.submission_manager, 1, None).unwrap_err();
}

#[test]
fn mock_e2e_same_tx_different_block_hash() {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let digest = runner
        .dispatcher
        .engine_manager
        .engine
        .store_component_bytes(COMPONENT_SQUARE_BYTES)
        .unwrap();

    let workflow_id = WorkflowId::new("workflow-1").unwrap();
    let contract_address = rand_address_evm();
    let chain = ChainKey::new("evm:anvil").unwrap();
    // Keccak256("NewTrigger(bytes)") from SimpleTrigger.NewTrigger
    let event_hash = {
        let bytes =
            const_hex::decode("86eacd23610d81706516de1ed0476c87772fdf939c7c771fbbd7f0230d619e68")
                .unwrap();
        wavs_types::ByteArray::try_from(bytes).unwrap()
    };
    let trigger = Trigger::EvmContractEvent {
        address: contract_address,
        chain: chain.clone(),
        event_hash,
    };

    let service = Service {
        name: "Test Service".to_string(),
        workflows: [(
            workflow_id.clone(),
            Workflow {
                trigger: trigger.clone(),
                component: Component::new(ComponentSource::Digest(digest.clone())),
                submit: Submit::Aggregator {
                    url: "http://example.com".to_string(),
                    component: Box::new(Component::new(ComponentSource::Digest(digest.clone()))),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]
        .into(),
        status: wavs_types::ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: chain.clone(),
            address: rand_address_evm(),
        },
    };

    runner.ctx.rt.block_on({
        let runner = runner.clone();
        let service = service.clone();
        async move { runner.dispatcher.add_service_direct(service).await.unwrap() }
    });

    let same_tx_hash = alloy_primitives::TxHash::from_slice(&[1u8; 32]);
    let block_hash_1 = alloy_primitives::B256::from_slice(&[2u8; 32]);
    let block_hash_2 = alloy_primitives::B256::from_slice(&[3u8; 32]);

    // ABI-encode event data: NewTrigger(bytes triggerData)
    // triggerData = abi.encode(TriggerInfo(triggerId, creator, data))
    let payload = SquareRequest { x: 3 }.to_vec();
    let trigger_info = TriggerInfo {
        triggerId: 1,
        creator: contract_address,
        data: payload.clone().into(),
    };
    let trigger_info_bytes: Vec<u8> = trigger_info.abi_encode();
    let event_data: Vec<u8> = trigger_info_bytes.abi_encode();
    let log_data = LogData::new(vec![event_hash.into_inner().into()], event_data.into()).unwrap();

    let make_action = |block_hash| TriggerAction {
        config: TriggerConfig {
            service_id: service.id(),
            workflow_id: workflow_id.clone(),
            trigger: trigger.clone(),
        },
        data: TriggerData::EvmContractEvent {
            block_hash,
            chain: chain.clone(),
            contract_address,
            log_data: log_data.clone(),
            tx_hash: same_tx_hash,
            block_number: 1,
            log_index: 0,
            block_timestamp: 0,
            tx_index: 0,
        },
    };

    let trigger_action_1 = make_action(block_hash_1);
    let trigger_action_2 = make_action(block_hash_2);

    // EventIds should differ due to different block hashes
    let event_id_1 = EventId::try_from((&service, &trigger_action_1)).unwrap();
    let event_id_2 = EventId::try_from((&service, &trigger_action_2)).unwrap();
    assert_ne!(event_id_1, event_id_2);

    // Send both triggers through the dispatcher and wait for submissions
    runner.ctx.rt.block_on({
        let runner = runner.clone();
        async move {
            runner
                .dispatcher
                .trigger_manager
                .send_dispatcher_commands(vec![
                    DispatcherCommand::Trigger(trigger_action_1),
                    DispatcherCommand::Trigger(trigger_action_2),
                ])
                .await
                .unwrap();
        }
    });

    wait_for_submission_messages(&runner.dispatcher.submission_manager, 2, None).unwrap();
}
