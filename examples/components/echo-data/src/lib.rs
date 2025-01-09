#[allow(warnings)]
mod bindings;
use bindings::{Contract, Guest};

struct Component;

impl Guest for Component {
    fn run(_contract: Contract, input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        Ok(input)
    }
}

bindings::export!(Component with_types_in bindings);
