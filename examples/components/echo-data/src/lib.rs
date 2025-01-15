use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use layer_wasi::{
    bindings::worlds::any_contract_event::{Guest, Input},
    export_any_contract_event_world,
};

struct Component;

impl Guest for Component {
    fn run(input: Input) -> std::result::Result<Vec<u8>, String> {
        let (trigger_id, req) =
            decode_trigger_event(input.event.into()).map_err(|e| e.to_string())?;

        if let Ok(input_str) = std::str::from_utf8(&req) {
            if input_str.contains("envvar:") {
                let env_var = input_str.split("envvar:").nth(1).unwrap();
                if let Ok(value) = std::env::var(env_var) {
                    return Ok(encode_trigger_output(trigger_id, value.as_bytes().to_vec()));
                } else {
                    return Err(format!("env var {} not found", env_var));
                }
            }
        }
        Ok(encode_trigger_output(trigger_id, req))
    }
}

export_any_contract_event_world!(Component);
