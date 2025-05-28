// these are like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected

use serde::{Deserialize, Serialize};
use utils::context::AppContext;
use wavs::test_utils::{address::rand_address_evm, mock_app::MockE2ETestRunner};
use wavs_types::{ComponentSource, Digest, ServiceID, WorkflowID};

const SQUARE: &[u8] = include_bytes!("../../../examples/build/components/square.wasm");
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct SquareIn {
    pub x: u64,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]

pub struct SquareOut {
    pub y: u64,
}

#[test]
fn mock_e2e_trigger_flow() {
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
                .store_component_bytes(SQUARE)
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
            runner
                .send_trigger(
                    &service_id,
                    &WorkflowID::default(),
                    &task_queue_address.into(),
                    &SquareIn { x: 21 },
                    "evm",
                )
                .await;
        }
    });

    // block and wait for triggers to go through the whole flow
    runner.dispatcher.submission.wait_for_messages(2).unwrap();

    // check the results
    let results: Vec<SquareOut> = runner
        .dispatcher
        .submission
        .received()
        .iter()
        .map(|msg| serde_json::from_slice(&msg.envelope.payload).unwrap())
        .collect();

    assert_eq!(results, vec![SquareOut { y: 9 }, SquareOut { y: 441 }]);
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
                .store_component_bytes(SQUARE)
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
            assert_eq!(services.digests.len(), 3);
            assert_eq!(services.services[0].id, service_id1);
            assert_eq!(services.services[1].id, service_id2);
            assert_eq!(services.services[2].id, service_id3);

            // add an orphaned digest
            let orphaned_digest = Digest::new(b"orphaned");
            runner
                .dispatcher
                .engine_manager
                .engine
                .store_component_from_source(&ComponentSource::Digest(orphaned_digest))
                .await
                .unwrap();

            let services = runner.list_services().await;
            assert_eq!(services.services.len(), 3);
            assert_eq!(services.digests.len(), 4);

            // selectively delete services 1 and 3, leaving just 2

            runner
                .delete_services(vec![service_id1.clone(), service_id3.clone()])
                .await;

            let services = runner.list_services().await;

            assert_eq!(services.services.len(), 1);
            assert_eq!(services.digests.len(), 4);
            assert_eq!(services.services[0].id, service_id2);

            // and make sure we can delete the last one but still get an empty list
            runner.delete_services(vec![service_id2.clone()]).await;

            let services = runner.list_services().await;

            assert!(services.services.is_empty());
            assert_eq!(services.digests.len(), 4);
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
                .store_component_bytes(SQUARE)
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
    runner
        .dispatcher
        .submission
        .wait_for_messages(1)
        .unwrap_err();
}
