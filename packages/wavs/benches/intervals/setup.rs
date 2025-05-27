use std::{
    num::NonZero,
    sync::{Arc, Mutex},
};

use opentelemetry::global::meter;
use tokio::sync::mpsc;
use utils::telemetry::Metrics;
use wavs::{apis::trigger::TriggerManager, triggers::core::CoreTriggerManager};
use wavs_benchmark_common::app_context::APP_CONTEXT;
use wavs_types::{ChainName, Trigger, TriggerAction, TriggerConfig};

// This is a convenience struct to initialize stuff and make it easier to pass around
pub struct Setup {
    pub chain_names: Vec<ChainName>,
    pub trigger_manager: CoreTriggerManager,
    pub action_receiver: Mutex<Option<mpsc::Receiver<TriggerAction>>>,
    pub config: SetupConfig,
}

#[derive(Clone, Copy)]
pub struct SetupConfig {
    // how many chains
    pub n_chains: u64,
    // how many blocks to launch triggers in per-chain
    pub n_blocks: u64,
    // how many triggers to launch in each block
    pub triggers_per_block: u64,
    // how many cycles to let the intervals run
    pub cycles: u64,
}

impl SetupConfig {
    pub fn description(&self) -> String {
        format!(
            "total triggers: {} (chains: {}, blocks: {}, triggers per block: {}, cycles: {})",
            self.total_triggers(),
            self.n_chains,
            self.n_blocks,
            self.triggers_per_block,
            self.cycles
        )
    }
    pub fn total_blocks(&self) -> u64 {
        self.n_blocks * self.cycles
    }

    pub fn total_triggers(&self) -> u64 {
        self.n_chains * self.total_blocks() * self.triggers_per_block
    }
}

impl Setup {
    pub fn new(setup_config: SetupConfig) -> Arc<Self> {
        let config = wavs::config::Config::default();
        let metrics = Metrics::new(&meter("wavs-benchmark"));

        let trigger_manager = CoreTriggerManager::new(&config, metrics.wavs.trigger).unwrap();
        let receiver = trigger_manager.start(APP_CONTEXT.clone()).unwrap();

        let mut chain_names = Vec::with_capacity(setup_config.n_chains as usize);

        let mut trigger_id = 1;
        for chain in 1..=setup_config.n_chains {
            let chain_name = ChainName::new(format!("wavs-benchmark-{chain}")).unwrap();
            for block in 1..=setup_config.n_blocks {
                for _ in 0..setup_config.triggers_per_block {
                    trigger_manager
                        .add_trigger(TriggerConfig {
                            service_id: wavs_types::ServiceID::new(format!(
                                "wavs-benchmark-{trigger_id}"
                            ))
                            .unwrap(),
                            workflow_id: wavs_types::WorkflowID::new(format!(
                                "wavs-benchmark-{trigger_id}"
                            ))
                            .unwrap(),
                            trigger: Trigger::BlockInterval {
                                chain_name: chain_name.clone(),
                                n_blocks: NonZero::new(setup_config.n_blocks as u32).unwrap(),
                                start_block: Some(NonZero::new(block).unwrap()),
                                end_block: None,
                            },
                        })
                        .unwrap();

                    trigger_id += 1;
                }
            }

            chain_names.push(chain_name);
        }

        Arc::new(Setup {
            trigger_manager,
            action_receiver: Mutex::new(Some(receiver)),
            chain_names,
            config: setup_config,
        })
    }
}
