// these are like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected

use wasmatic::{
    apis::ID,
    context::AppContext,
    engine::runner::EngineRunner,
    test_utils::{
        chain::MOCK_TASK_QUEUE_ADDRESS,
        mock::{BigSquare, MockE2ETestRunner, SquareIn, SquareOut},
    },
    Digest,
};

#[test]
fn mock_e2e_trigger_flow() {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let service_id = ID::new("service1").unwrap();
    let workflow_id = ID::new("default").unwrap();

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
                    &MOCK_TASK_QUEUE_ADDRESS,
                    BigSquare,
                )
                .await;
        }
    });

    // now pretend like we're reading some triggers off the chain
    // this spawned into the async runtime, so it's sortof like the real TriggerManager
    runner.ctx.rt.spawn({
        let runner = runner.clone();
        async move {
            runner
                .dispatcher
                .triggers
                .send_trigger(
                    &service_id,
                    &workflow_id,
                    &MOCK_TASK_QUEUE_ADDRESS,
                    &SquareIn { x: 3 },
                )
                .await;
            runner
                .dispatcher
                .triggers
                .send_trigger(
                    &service_id,
                    &workflow_id,
                    &MOCK_TASK_QUEUE_ADDRESS,
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

            assert!(services.apps.is_empty());
            assert!(services.digests.is_empty());

            // add services in order
            let service_id1 = ID::new("service1").unwrap();
            let digest1 = Digest::new(b"wasm1");

            let service_id2 = ID::new("service2").unwrap();
            let digest2 = Digest::new(b"wasm2");

            let service_id3 = ID::new("service3").unwrap();
            let digest3 = Digest::new(b"wasm3");

            for (service_id, digest) in [
                (&service_id1, digest1),
                (&service_id2, digest2),
                (&service_id3, digest3),
            ] {
                runner
                    .create_service(
                        service_id.clone(),
                        digest.clone(),
                        &MOCK_TASK_QUEUE_ADDRESS,
                        BigSquare,
                    )
                    .await;
            }

            let services = runner.list_services().await;

            assert_eq!(services.apps.len(), 3);
            assert_eq!(services.digests.len(), 3);
            assert_eq!(services.apps[0].name, service_id1.to_string());
            assert_eq!(services.apps[1].name, service_id2.to_string());
            assert_eq!(services.apps[2].name, service_id3.to_string());

            // add an orphaned digest
            let orphaned_digest = Digest::new(b"orphaned");
            runner
                .dispatcher
                .engine
                .engine()
                .register(&orphaned_digest, BigSquare);

            let services = runner.list_services().await;
            assert_eq!(services.apps.len(), 3);
            assert_eq!(services.digests.len(), 4);

            // selectively delete services 1 and 3, leaving just 2

            runner
                .delete_services(vec![service_id1.clone(), service_id3.clone()])
                .await;

            let services = runner.list_services().await;

            assert_eq!(services.apps.len(), 1);
            assert_eq!(services.digests.len(), 4);
            assert_eq!(services.apps[0].name, service_id2.to_string());

            // and make sure we can delete the last one but still get an empty list
            runner
                .delete_services(vec![service_id1.clone(), service_id3.clone()])
                .await;

            runner.delete_services(vec![service_id2.clone()]).await;

            let services = runner.list_services().await;

            assert!(services.apps.is_empty());
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
            let service_id = ID::new("service").unwrap();
            let digest = Digest::new(b"wasm");

            runner
                .create_service(
                    service_id.clone(),
                    digest.clone(),
                    &MOCK_TASK_QUEUE_ADDRESS,
                    BigSquare,
                )
                .await;

            let SquareOut { y } = runner.test_service(service_id, SquareIn { x: 3 }).await;

            assert_eq!(y, 9);
        }
    })
}
