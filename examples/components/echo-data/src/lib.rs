#[allow(warnings)]
mod bindings;
use bindings::{Contract, Guest};
use example_helpers::trigger::{decode_trigger_input, encode_trigger_output};

struct Component;

impl Guest for Component {
    fn run(_contract: Contract, input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        let (trigger_id, input) = decode_trigger_input(input)?;
        Ok(encode_trigger_output(trigger_id, input))
    }
}

bindings::export!(Component with_types_in bindings);
