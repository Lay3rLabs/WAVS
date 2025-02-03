use example_helpers::{
    bindings::{
        compat::TriggerData,
        world::{Guest, TriggerAction},
    },
    export_layer_trigger_world,
};
struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<u8>, String> {
        match trigger_action.data {
            TriggerData::Raw(data) => {
                if let Ok(input_str) = std::str::from_utf8(&data) {
                    if input_str.contains("envvar:") {
                        let env_var = input_str.split("envvar:").nth(1).unwrap();
                        if let Ok(value) = std::env::var(env_var) {
                            return Ok(value.as_bytes().to_vec());
                        } else {
                            return Err(format!("env var {} not found", env_var));
                        }
                    }
                }
                Ok(data)
            }
            _ => Err("expected raw trigger data".to_string()),
        }
    }
}

export_layer_trigger_world!(Component);
