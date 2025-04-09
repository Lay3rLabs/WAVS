use example_helpers::{
    bindings::{
        compat::TriggerData,
        world::{host, Guest, TriggerAction, WasmResponse},
    },
    export_layer_trigger_world,
};
use wstd::runtime::block_on;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        // For internal testing purposes
        if let Some(n) = host::config_var("sleep-seconds") {
            let n = n
                .parse::<u64>()
                .map_err(|e| format!("invalid sleep-seconds {e:?}"))?;

            match host::config_var("sleep-kind").as_deref() {
                Some("async") => {
                    block_on(async move {
                        wstd::task::sleep(wstd::time::Duration::from_secs(n)).await;
                    });
                }
                Some("sync") => {
                    std::thread::sleep(std::time::Duration::from_secs(n));
                }
                _ => {
                    return Err(
                        "invalid or missing 'sleep-kind', must be 'async' or 'sync'".to_string()
                    );
                }
            }
        }

        match trigger_action.data {
            TriggerData::Raw(data) => {
                if let Ok(input_str) = std::str::from_utf8(&data) {
                    if input_str.contains("envvar:") {
                        let env_var = input_str.split("envvar:").nth(1).unwrap();
                        if let Ok(value) = std::env::var(env_var) {
                            return Ok(Some(WasmResponse {
                                payload: value.as_bytes().to_vec(),
                                ordering: None,
                            }));
                        } else {
                            return Err(format!("env var {} not found", env_var));
                        }
                    } else if input_str.contains("configvar:") {
                        let config_var = input_str.split("configvar:").nth(1).unwrap();
                        if let Some(value) = host::config_var(config_var) {
                            return Ok(Some(WasmResponse {
                                payload: value.as_bytes().to_vec(),
                                ordering: None,
                            }));
                        } else {
                            return Err(format!("config var {} not found", config_var));
                        }
                    }
                }

                Ok(Some(WasmResponse {
                    payload: data,
                    ordering: None,
                }))
            }
            _ => Err("expected raw trigger data".to_string()),
        }
    }
}

export_layer_trigger_world!(Component);
