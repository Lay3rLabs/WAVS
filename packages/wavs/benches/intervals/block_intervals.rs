use std::sync::Arc;

use criterion::Criterion;
use tokio::sync::oneshot;
use wavs_benchmark_common::app_context::APP_CONTEXT;

use crate::setup::{Setup, SetupConfig};

pub fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("block intervals");
    group.measurement_time(std::time::Duration::from_secs(180));
    group.sample_size(10);

    let config = SetupConfig {
        n_chains: 7,
        n_blocks: 1000,
        triggers_per_block: 100,
        cycles: 5,
    };

    group.bench_function(config.description(), move |b| {
        b.iter_with_setup(|| Setup::new(config), run_simulation);
    });

    group.finish();
}

fn run_simulation(setup: Arc<Setup>) {
    // This channel will signal when the simulation is finished
    let (finished_sender, finished_receiver) = oneshot::channel::<u64>();

    // First spawn up a thread that *listens* for the actions as they come in
    // of course, nothing will be sent until we start processing blocks
    std::thread::spawn({
        let setup = setup.clone();
        move || {
            let mut count = 0;

            while setup.trigger_to_dispatcher_rx.recv().is_ok() {
                count += 1;
                if count == setup.config.total_triggers() {
                    // all done, send the finished signal!
                    finished_sender.send(count).unwrap();
                    break;
                }
            }
        }
    });

    // Now spawn up a thread that will process the blocks
    // and send the actions to the action receiver
    std::thread::spawn({
        let setup = setup.clone();
        move || {
            for block_height in 0..=setup.config.total_blocks() {
                for chain in setup.chains.clone() {
                    let commands = setup.trigger_manager.process_blocks(chain, block_height);
                    setup
                        .trigger_manager
                        .send_dispatcher_commands(commands)
                        .unwrap();
                }
            }
        }
    });

    let total_triggers = setup.config.total_triggers();

    // Wait for the finished signal. This will effectively keep the simulation running
    let received_count = APP_CONTEXT
        .rt
        .block_on(async move { finished_receiver.await.unwrap() });

    assert_eq!(received_count, total_triggers);

    println!("Processed {received_count} triggers");
}
