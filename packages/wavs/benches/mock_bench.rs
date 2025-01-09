use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use utils::{ServiceID, WorkflowID};
use wavs::{
    test_utils::{
        address::rand_address_eth,
        mock::{BigSquare, MockE2ETestRunner, SquareIn},
    },
    AppContext, Digest,
};

pub fn criterion_benchmark(c: &mut Criterion) {
    let runner = MockE2ETestRunner::new(AppContext::new());

    let service_id = ServiceID::new("default").unwrap();
    let workflow_id = WorkflowID::new("default").unwrap();
    let task_queue_address = rand_address_eth();

    // block and wait for creating the service
    runner.ctx.rt.block_on({
        let runner = runner.clone();
        let service_id = service_id.clone();

        async move {
            let digest = Digest::new(b"wasm");
            runner
                .create_service_simple(service_id.clone(), digest, BigSquare)
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
                async move {
                    for i in 1..=N_TRIGGERS {
                        runner
                            .dispatcher
                            .triggers
                            .send_trigger(
                                &service_id,
                                &workflow_id,
                                &task_queue_address,
                                &SquareIn { x: i as u64 },
                                "eth",
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
