use std::sync::Arc;

use alloy_sol_types::SolValue;
use example_types::{SquareRequest, SquareResponse};
use utils::test_utils::test_contracts::ISimpleSubmit::DataWithId;
use utils::{
    context::AppContext,
    test_utils::{
        address::{rand_address_cosmos, rand_address_evm},
        mock_engine::COMPONENT_SQUARE_BYTES,
    },
};
use wavs::dispatcher::DispatcherCommand;
use wavs::init_tracing_tests;
use wavs_types::{
    ChainName, Component, ComponentSource, EvmContractSubmission, Service, ServiceManager,
    ServiceStatus, Submit, Workflow, WorkflowID,
};
mod wavs_systems;
use wavs_systems::{
    mock_app::MockE2ETestRunner,
    mock_submissions::wait_for_submission_messages,
    mock_trigger_manager::{mock_cosmos_event_trigger, mock_real_trigger_action},
};

/// Simple test to check that the dispatcher can handle the full pipeline
#[test]
fn dispatcher_pipeline() {
    init_tracing_tests();

    let data_dir = tempfile::tempdir().unwrap();

    // Prepare two actions to be squared
    let workflow_id = WorkflowID::new("workflow1").unwrap();
    let chain_name = "cosmos".to_string();

    let ctx = AppContext::new();
    let dispatcher = Arc::new(MockE2ETestRunner::create_dispatcher(ctx.clone(), &data_dir));

    // Register the square component
    let digest = dispatcher
        .engine_manager
        .engine
        .store_component_bytes(COMPONENT_SQUARE_BYTES)
        .unwrap();

    // Register a service to handle this action
    let service = Service {
        name: "Big Square AVS".to_string(),
        workflows: [(
            workflow_id.clone(),
            Workflow {
                component: Component::new(ComponentSource::Digest(digest)),
                trigger: mock_cosmos_event_trigger(),
                submit: Submit::Aggregator {
                    url: "http://example.com/aggregator".to_string(),
                    component: None,
                    evm_contracts: Some(vec![EvmContractSubmission {
                        chain_name: chain_name.parse().unwrap(),
                        address: rand_address_evm(),
                        max_gas: None,
                    }]),
                },
            },
        )]
        .into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain_name: ChainName::new("evm").unwrap(),
            address: rand_address_evm(),
        },
    };

    let contract_address = rand_address_cosmos();
    let actions = vec![
        mock_real_trigger_action(
            service.id(),
            &workflow_id,
            &contract_address,
            &SquareRequest::new(3),
            &chain_name,
        ),
        mock_real_trigger_action(
            service.id(),
            &workflow_id,
            &contract_address,
            &SquareRequest::new(21),
            &chain_name,
        ),
    ];

    // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
    std::thread::spawn({
        let dispatcher = dispatcher.clone();
        let ctx = ctx.clone();
        move || {
            dispatcher.start(ctx).unwrap();
        }
    });

    ctx.rt.block_on(async {
        dispatcher.add_service_direct(service).await.unwrap();
        dispatcher
            .trigger_manager
            .send_dispatcher_commands(actions.into_iter().map(DispatcherCommand::Trigger))
            .await
            .unwrap();
    });

    // check that the events were properly handled and arrived at submission
    wait_for_submission_messages(&dispatcher.submission_manager, 2, None).unwrap();
    let processed = dispatcher.submission_manager.get_debug_packets();
    assert_eq!(processed.len(), 2);

    let payload_1: DataWithId = DataWithId::abi_decode(&processed[0].envelope.payload).unwrap();
    let data_1: SquareResponse = serde_json::from_slice(&payload_1.data).unwrap();

    let payload_2: DataWithId = DataWithId::abi_decode(&processed[1].envelope.payload).unwrap();
    let data_2: SquareResponse = serde_json::from_slice(&payload_2.data).unwrap();

    // Check the payloads
    assert_eq!(data_1, SquareResponse::new(9));

    assert_eq!(data_2, SquareResponse::new(441));
}
