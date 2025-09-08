// these are like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected

use example_types::SquareRequest;
use utils::{
    context::AppContext,
    test_utils::{
        address::{rand_address_cosmos, rand_address_evm},
        mock_engine::{COMPONENT_ECHO_DATA_BYTES, COMPONENT_SQUARE_BYTES},
    },
};
mod wavs_systems;
use wavs_systems::{mock_app::MockE2ETestRunner, mock_submissions::wait_for_submission_messages};
use wavs_types::{ComponentSource, ServiceManager, WorkflowId};

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
