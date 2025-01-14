#[allow(warnings)]
mod bindings;
use bindings::Guest;

struct Component;

impl Guest for Component {
    fn process_eth_trigger(input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        // for internal testing
        if let Ok(input_str) = std::str::from_utf8(&input) {
            if input_str.contains("envvar:") {
                let env_var = input_str.split("envvar:").nth(1).unwrap();
                if let Ok(value) = std::env::var(env_var) {
                    return Ok(value.as_bytes().to_vec());
                } else {
                    return Err(format!("env var {} not found", env_var));
                }
            }
        }

        Ok(input)
    }
}

bindings::export!(Component with_types_in bindings);
