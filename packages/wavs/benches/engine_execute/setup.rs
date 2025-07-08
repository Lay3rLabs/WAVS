use std::{collections::BTreeMap, sync::Arc};

use wavs_benchmark_common::engine_setup::EngineSetup;
use wavs_types::TriggerAction;

#[derive(Clone)]
pub struct ExecuteConfig {
    /// Number of executions
    pub n_executions: u64,
    /// Configuration for async component calls
    pub sleep_config: Option<SleepConfig>,
}

#[derive(Clone)]
pub struct SleepConfig {
    pub sleep_ms: u64,
    pub sleep_kind: String,
}

impl ExecuteConfig {
    pub fn description(&self) -> String {
        format!(
            "{}{} executions",
            self.n_executions,
            self.sleep_config
                .as_ref()
                .map(|x| format!(" {} sleep", x.sleep_kind))
                .unwrap_or_default()
        )
    }
}

/// SystemHandle provides the setup and infrastructure needed for MultiEngineRunner benchmarks
/// This struct combines an EngineHandle with a MultiEngineRunner to test system-level throughput
pub struct ExecuteSetup {
    pub engine_setup: Arc<EngineSetup>,
    #[allow(clippy::type_complexity)]
    pub trigger_actions: std::sync::Mutex<Option<Vec<(TriggerAction, Vec<u8>)>>>,
}

impl ExecuteSetup {
    pub fn new(execute_config: ExecuteConfig) -> Arc<Self> {
        let config = if let Some(sleep_config) = execute_config.sleep_config {
            BTreeMap::from_iter([
                ("sleep-ms".to_string(), sleep_config.sleep_ms.to_string()),
                ("sleep-kind".to_string(), sleep_config.sleep_kind),
            ])
        } else {
            BTreeMap::new()
        };
        let engine_setup = EngineSetup::new(config);

        let trigger_actions = (1..=execute_config.n_executions)
            .map(|execution_count| {
                let echo_data = format!("Execution number {execution_count}").into_bytes();
                let action = engine_setup.create_trigger_action(echo_data.clone());
                (action, echo_data)
            })
            .collect::<Vec<_>>();

        Arc::new(Self {
            engine_setup,
            trigger_actions: std::sync::Mutex::new(Some(trigger_actions)),
        })
    }
}
