// these are like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected

use utils::{
    context::AppContext,
    test_utils::{
        address::{rand_address_cosmos, rand_address_evm},
        mock_engine::{SquareIn, COMPONENT_ECHO_DATA, COMPONENT_SQUARE},
    },
};
mod wavs_systems;
use wavs_systems::{mock_app::MockE2ETestRunner, mock_submissions::wait_for_submission_messages};
use wavs_types::{ComponentSource, ServiceID, WorkflowID};

#[test]
fn mock_e2e_trigger_flow() {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let service_id = ServiceID::new("service1").unwrap();
    let task_queue_address = rand_address_cosmos();

    // block and wait for creating the service
    runner.ctx.rt.block_on({
        let runner = runner.clone();
        let service_id = service_id.clone();

        async move {
            let digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_SQUARE)
                .unwrap();
            runner
                .create_service(service_id.clone(), ComponentSource::Digest(digest))
                .await;
        }
    });

    // now pretend like we're reading some triggers off the chain
    runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            runner
                .send_trigger(
                    &service_id,
                    &WorkflowID::default(),
                    &task_queue_address.clone(),
                    &SquareIn { x: 3 },
                    "evm",
                )
                .await;
            runner
                .send_trigger(
                    &service_id,
                    &WorkflowID::default(),
                    &task_queue_address,
                    &SquareIn { x: 21 },
                    "evm",
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
            let services = runner.list_services().await;

            assert!(services.services.is_empty());
            assert!(services.digests.is_empty());

            // add services in order
            let service_id1 = ServiceID::new("service1").unwrap();
            let digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_SQUARE)
                .unwrap();

            let service_id2 = ServiceID::new("service2").unwrap();

            let service_id3 = ServiceID::new("service3").unwrap();

            for service_id in [&service_id1, &service_id2, &service_id3] {
                runner
                    .create_service(service_id.clone(), ComponentSource::Digest(digest.clone()))
                    .await;
            }

            let services = runner.list_services().await;

            assert_eq!(services.services.len(), 3);
            assert_eq!(services.digests.len(), 1);
            assert_eq!(services.services[0].id, service_id1);
            assert_eq!(services.services[1].id, service_id2);
            assert_eq!(services.services[2].id, service_id3);

            // add an orphaned digest
            let _orphaned_digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_ECHO_DATA)
                .unwrap();

            let services = runner.list_services().await;
            assert_eq!(services.services.len(), 3);
            assert_eq!(services.digests.len(), 2);

            // selectively delete services 1 and 3, leaving just 2

            runner
                .delete_services(vec![service_id1.clone(), service_id3.clone()])
                .await;

            let services = runner.list_services().await;

            assert_eq!(services.services.len(), 1);
            assert_eq!(services.digests.len(), 2);
            assert_eq!(services.services[0].id, service_id2);

            // and make sure we can delete the last one but still get an empty list
            runner.delete_services(vec![service_id2.clone()]).await;

            let services = runner.list_services().await;

            assert!(services.services.is_empty());
            assert_eq!(services.digests.len(), 2);
        }
    });
}

#[test]
fn mock_e2e_component_none() {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let service_id = ServiceID::new("service1").unwrap();
    let task_queue_address = rand_address_evm();

    // block and wait for creating the service
    runner.ctx.rt.block_on({
        let runner = runner.clone();
        let service_id = service_id.clone();

        async move {
            let digest = runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_bytes(COMPONENT_SQUARE)
                .unwrap();

            runner
                .create_service(service_id.clone(), ComponentSource::Digest(digest))
                .await;
        }
    });

    // now pretend like we're reading some triggers off the chain
    runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            runner
                .send_trigger(
                    &service_id,
                    &WorkflowID::default(),
                    &task_queue_address.into(),
                    &SquareIn { x: 3 },
                    "evm",
                )
                .await;
        }
    });

    // this _should_ error because submission is not fired
    wait_for_submission_messages(&runner.dispatcher.submission_manager, 1, None).unwrap_err();
}
