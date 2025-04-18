use example_helpers::bindings::world::{host, Guest, TriggerAction, WasmResponse};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        let (trigger_id, req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

        if let Ok(input_str) = std::str::from_utf8(&req) {
            if input_str.contains("envvar:") {
                let env_var = input_str.split("envvar:").nth(1).unwrap();
                if let Ok(value) = std::env::var(env_var) {
                    return Ok(Some(encode_trigger_output(trigger_id, value.as_bytes())));
                } else {
                    return Err(format!("env var {} not found", env_var));
                }
            } else if input_str.contains("configvar:") {
                let config_var = input_str.split("configvar:").nth(1).unwrap();
                if let Some(value) = host::config_var(config_var) {
                    return Ok(Some(encode_trigger_output(trigger_id, value.as_bytes())));
                } else {
                    return Err(format!("config var {} not found", config_var));
                }
            }
        }
        Ok(Some(encode_trigger_output(trigger_id, req)))
    }
}

export_layer_trigger_world!(Component);
