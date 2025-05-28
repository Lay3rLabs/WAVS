use example_helpers::bindings::compat::TriggerData;
use example_helpers::bindings::world::{host, Guest, TriggerAction, WasmResponse};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use wstd::runtime::block_on;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        if let Some(n) = host::config_var("sleep-ms") {
            let n = n
                .parse::<u64>()
                .map_err(|e| format!("invalid sleep-ms {e:?}"))?;

            match host::config_var("sleep-kind").as_deref() {
                Some("async") => {
                    block_on(async move {
                        wstd::task::sleep(wstd::time::Duration::from_millis(n)).await;
                    });
                }
                Some("sync") => {
                    std::thread::sleep(std::time::Duration::from_millis(n));
                }
                _ => {
                    return Err(
                        "invalid or missing 'sleep-kind', must be 'async' or 'sync'".to_string()
                    );
                }
            }
        }

        let (maybe_trigger_id, data) = match trigger_action.data {
            TriggerData::EvmContractEvent(_) | TriggerData::CosmosContractEvent(_) => {
                let (trigger_id, data) =
                    decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

                Ok((Some(trigger_id), data))
            }
            TriggerData::Raw(data) => Ok((None, data)),
            _ => Err("expected trigger data".to_string()),
        }?;

        if let Ok(input_str) = std::str::from_utf8(&data) {
            if input_str.contains("envvar:") {
                let env_var = input_str.split("envvar:").nth(1).unwrap();
                if let Ok(value) = std::env::var(env_var) {
                    if let Some(trigger_id) = maybe_trigger_id {
                        return Ok(Some(encode_trigger_output(trigger_id, value)));
                    }
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
                    if let Some(trigger_id) = maybe_trigger_id {
                        return Ok(Some(encode_trigger_output(trigger_id, value)));
                    }
                    return Ok(Some(WasmResponse {
                        payload: value.as_bytes().to_vec(),
                        ordering: None,
                    }));
                } else {
                    return Err(format!("config var {} not found", config_var));
                }
            }
        }

        if let Some(trigger_id) = maybe_trigger_id {
            return Ok(Some(encode_trigger_output(trigger_id, data)));
        }
        Ok(Some(WasmResponse {
            payload: data,
            ordering: None,
        }))
    }
}

export_layer_trigger_world!(Component);
