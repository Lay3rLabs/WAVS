#![cfg(feature = "dev")]
use std::sync::Arc;

use alloy_sol_types::SolValue;
use example_types::{SquareRequest, SquareResponse};
use utils::init_tracing_tests;
use utils::test_utils::test_contracts::ISimpleSubmit::DataWithId;
use utils::{
    context::AppContext,
    test_utils::{
        address::{rand_address_cosmos, rand_address_evm},
        mock_engine::COMPONENT_SQUARE_BYTES,
    },
};
use wavs::dispatcher::DispatcherCommand;
use wavs_types::{
    Component, ComponentSource, Service, ServiceManager, ServiceStatus, SignatureKind, Submit,
    Workflow, WorkflowId,
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
    let workflow_id = WorkflowId::new("workflow1").unwrap();

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
                component: Component::new(ComponentSource::Digest(digest.clone())),
                trigger: mock_cosmos_event_trigger(),
                submit: Submit::Aggregator {
                    component: Box::new(Component::new(ComponentSource::Digest(digest))),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]
        .into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:anvil".parse().unwrap(),
            address: rand_address_evm(),
        },
    };

    let contract_address = rand_address_cosmos();
    let actions = vec![
        mock_real_trigger_action(
            service.id(),
            workflow_id.to_string().as_str(),
            &contract_address.clone().into(),
            &SquareRequest::new(3),
            service.manager.chain().to_string().as_str(),
        ),
        mock_real_trigger_action(
            service.id(),
            workflow_id.to_string().as_str(),
            &contract_address.into(),
            &SquareRequest::new(21),
            service.manager.chain().to_string().as_str(),
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
    });
    dispatcher
        .trigger_manager
        .send_dispatcher_commands(actions.into_iter().map(DispatcherCommand::Trigger))
        .unwrap();

    // check that the events were properly handled and arrived at submission
    wait_for_submission_messages(&dispatcher.submission_manager, 2, None).unwrap();
    let processed = dispatcher.submission_manager.get_debug_submissions();

    assert_eq!(processed.len(), 2);

    // eh, just happens to be the order we want

    let payload_1: DataWithId = DataWithId::abi_decode(&processed[0].envelope.payload).unwrap();
    let data_1: SquareResponse = serde_json::from_slice(&payload_1.data).unwrap();

    let payload_2: DataWithId = DataWithId::abi_decode(&processed[1].envelope.payload).unwrap();
    let data_2: SquareResponse = serde_json::from_slice(&payload_2.data).unwrap();

    // Check the payloads
    if data_1 == SquareResponse::new(9) {
        assert_eq!(data_2, SquareResponse::new(441));
    } else {
        assert_eq!(data_1, SquareResponse::new(441));
        assert_eq!(data_2, SquareResponse::new(9));
    }
}
