use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use wavs::{
    apis::ID,
    context::AppContext,
    test_utils::{
        address::rand_address_eth,
        mock::{BigSquare, MockE2ETestRunner, SquareIn},
    },
    Digest,
};

pub fn criterion_benchmark(c: &mut Criterion) {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let service_id = ID::new("default").unwrap();
    let workflow_id = ID::new("default").unwrap();
    let task_queue_address = rand_address_eth();
    let task_queue_erc1271 = rand_address_eth();

    // block and wait for creating the service
    runner.ctx.rt.block_on({
        let runner = runner.clone();
        let service_id = service_id.clone();
        let task_queue_address = task_queue_address.clone();
        let task_queue_erc1271 = task_queue_erc1271.clone();

        async move {
            let digest = Digest::new(b"wasm");
            runner
                .create_service_simple(
                    service_id.clone(),
                    digest,
                    &task_queue_address,
                    &task_queue_erc1271,
                    BigSquare,
                )
                .await;
        }
    });

    c.bench_function("mock_bench", |b| {
        // Run the benchmarks
        b.iter(|| {
            const N_TRIGGERS: usize = 1;

            let pre_submission_count = runner.dispatcher.submission.received().len();

            runner.ctx.rt.spawn({
                let runner = runner.clone();
                let service_id = service_id.clone();
                let workflow_id = workflow_id.clone();
                let task_queue_address = task_queue_address.clone();
                let task_queue_erc1271 = task_queue_erc1271.clone();
                async move {
                    for i in 1..=N_TRIGGERS {
                        runner
                            .dispatcher
                            .triggers
                            .send_trigger(
                                &service_id,
                                &workflow_id,
                                &task_queue_address,
                                &task_queue_erc1271,
                                &SquareIn { x: i as u64 },
                            )
                            .await;
                    }
                }
            });

            let submission_count_target = pre_submission_count + N_TRIGGERS;

            // FIXME
            runner
                .dispatcher
                .submission
                .wait_for_messages_timeout(submission_count_target, Duration::from_secs(60))
                .unwrap();
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
