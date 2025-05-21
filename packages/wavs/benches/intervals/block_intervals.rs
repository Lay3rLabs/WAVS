use std::sync::Arc;

use criterion::Criterion;
use tokio::sync::oneshot;

use crate::handle::{Handle, HandleConfig, APP_CONTEXT};

pub fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("block intervals");
    group.measurement_time(std::time::Duration::from_secs(180));
    group.sample_size(10);

    let config = HandleConfig {
        n_chains: 7,
        n_blocks: 1000,
        triggers_per_block: 100,
        cycles: 5,
    };

    group.bench_function(config.description(), move |b| {
        b.iter_with_setup(|| Handle::new(config), run_simulation);
    });

    group.finish();
}

fn run_simulation(handle: Arc<Handle>) {
    // This channel will signal when the simulation is finished
    let (finished_sender, finished_receiver) = oneshot::channel::<u64>();

    // First spawn up a thread that *listens* for the actions as they come in
    // of course, nothing will be sent until we start processing blocks
    std::thread::spawn({
        let handle = handle.clone();
        move || {
            APP_CONTEXT.rt.block_on(async move {
                let mut count = 0;
                // take out the action receiver so we can listen to it
                let mut receiver = handle.action_receiver.lock().unwrap().take().unwrap();

                while receiver.recv().await.is_some() {
                    count += 1;
                    if count == handle.config.total_triggers() {
                        // all done, send the finished signal!
                        finished_sender.send(count).unwrap();
                        break;
                    }
                }
            });
        }
    });

    // Now spawn up a thread that will process the blocks
    // and send the actions to the action receiver
    std::thread::spawn({
        let handle = handle.clone();
        move || {
            APP_CONTEXT.rt.block_on(async move {
                for block_height in 0..=handle.config.total_blocks() {
                    for chain_name in handle.chain_names.clone() {
                        let actions = handle
                            .trigger_manager
                            .process_blocks(chain_name, block_height);
                        for action in actions {
                            handle
                                .trigger_manager
                                .action_sender
                                .send(action)
                                .await
                                .unwrap();
                        }
                    }
                }
            });
        }
    });

    let total_triggers = handle.config.total_triggers();

    // Wait for the finished signal. This will effectively keep the simulation running
    let received_count = APP_CONTEXT
        .rt
        .block_on(async move { finished_receiver.await.unwrap() });

    assert_eq!(received_count, total_triggers);

    println!("Processed {} triggers", received_count);
}
