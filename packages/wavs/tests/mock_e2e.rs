// these are like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected

use utils::{ServiceID, WorkflowID};
use wavs::{
    apis::dispatcher::{AllowedHostPermission, Permissions, ServiceConfig},
    engine::runner::EngineRunner,
    test_utils::{
        address::rand_address_eth,
        mock::{BigSquare, MockE2ETestRunner, SquareIn, SquareOut},
    },
    AppContext, Digest,
};

#[test]
fn mock_e2e_trigger_flow() {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let service_id = ServiceID::new("service1").unwrap();
    let workflow_id = WorkflowID::new("test-workflow").unwrap();
    let task_queue_address = rand_address_eth();
    let config = ServiceConfig {
        workflow_id: workflow_id.clone(),
        ..Default::default()
    };

    // block and wait for creating the service
    runner.ctx.rt.block_on({
        let runner = runner.clone();
        let service_id = service_id.clone();

        async move {
            let digest = Digest::new(b"wasm");

            runner
                .create_service(
                    service_id.clone(),
                    digest,
                    Permissions::default(),
                    config,
                    BigSquare,
                )
                .await;
        }
    });

    // now pretend like we're reading some triggers off the chain
    // this spawned into the async runtime, so it's sortof like the real TriggerManager
    runner.ctx.rt.spawn({
        let runner = runner.clone();
        let task_queue_address = task_queue_address.clone();

        async move {
            runner
                .dispatcher
                .triggers
                .send_trigger(
                    &service_id,
                    &workflow_id,
                    &task_queue_address,
                    &SquareIn { x: 3 },
                )
                .await;
            runner
                .dispatcher
                .triggers
                .send_trigger(
                    &service_id,
                    &workflow_id,
                    &task_queue_address,
                    &SquareIn { x: 21 },
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
        .map(|msg| serde_json::from_slice(&msg.wasm_result).unwrap())
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
            let digest1 = Digest::new(b"wasm1");

            let service_id2 = ServiceID::new("service2").unwrap();
            let digest2 = Digest::new(b"wasm2");

            let service_id3 = ServiceID::new("service3").unwrap();
            let digest3 = Digest::new(b"wasm3");

            for (service_id, digest) in [
                (&service_id1, digest1),
                (&service_id2, digest2),
                (&service_id3, digest3),
            ] {
                runner
                    .create_service_simple(service_id.clone(), digest.clone(), BigSquare)
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
                .engine
                .engine()
                .register(&orphaned_digest, BigSquare);

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
            runner
                .delete_services(vec![service_id1.clone(), service_id3.clone()])
                .await;

            runner.delete_services(vec![service_id2.clone()]).await;

            let services = runner.list_services().await;

            assert!(services.services.is_empty());
            assert_eq!(services.digests.len(), 4);
        }
    });
}

#[test]
fn mock_e2e_service_test() {
    let runner = MockE2ETestRunner::new(AppContext::new());
    // block and wait for creating the service

    runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            // add services in order
            let service_id = ServiceID::new("service").unwrap();
            let digest = Digest::new(b"wasm");

            runner
                .create_service_simple(service_id.clone(), digest.clone(), BigSquare)
                .await;

            let SquareOut { y } = runner.test_service(service_id, SquareIn { x: 3 }).await;

            assert_eq!(y, 9);
        }
    })
}

#[test]
fn mock_e2e_service_settings() {
    let runner = MockE2ETestRunner::new(AppContext::new());
    // block and wait for creating the service

    runner.ctx.rt.block_on({
        let runner = runner.clone();

        async move {
            let service_id = ServiceID::new("service").unwrap();
            let digest = Digest::new(b"wasm");

            let permissions = Permissions {
                allowed_http_hosts: AllowedHostPermission::Only(vec!["example.com".to_string()]),
                ..Default::default()
            };

            let config = ServiceConfig::default();

            runner
                .create_service(
                    service_id.clone(),
                    digest.clone(),
                    permissions.clone(),
                    config.clone(),
                    BigSquare,
                )
                .await;

            let services = runner.list_services().await;

            assert_eq!(services.services[0].permissions, permissions);
        }
    })
}
