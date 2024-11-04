// these are like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected

use wasmatic::{
    apis::ID,
    context::AppContext,
    test_utils::{
        chain::MOCK_TASK_QUEUE_ADDRESS,
        mock::{BigSquare, MockE2ETestRunner, SquareIn},
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
    let results: Vec<serde_json::Value> = runner
        .dispatcher
        .submission
        .received()
        .iter()
        .map(|msg| serde_json::from_slice(&msg.wasm_result).unwrap())
        .collect();

    tracing::info!("results: {:?}", results);

    assert_eq!(
        results,
        vec![serde_json::json!({"y": 9}), serde_json::json!({"y": 441})]
    );
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


            let service_id1 = ID::new("service1").unwrap();
            let service_id2 = ID::new("service2").unwrap();
            let digest = Digest::new(b"wasm1");
            runner
                .create_service(
                    service_id1.clone(),
                    digest,
                    &MOCK_TASK_QUEUE_ADDRESS,
                    BigSquare,
                )
                .await;

            let digest = Digest::new(b"wasm2");
            runner
                .create_service(
                    service_id2.clone(),
                    digest,
                    &MOCK_TASK_QUEUE_ADDRESS,
                    BigSquare,
                )
                .await;


        }
    });

}