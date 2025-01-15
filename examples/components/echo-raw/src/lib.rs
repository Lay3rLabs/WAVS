#[allow(warnings, dead_code)]
use layer_wasi::bindings::worlds::raw::Guest;
use layer_wasi::export_raw_world;
struct Component;

impl Guest for Component {
    fn run(input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        Ok(input)
    }
}

export_raw_world!(Component);
