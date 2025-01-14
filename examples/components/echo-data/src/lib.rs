#[allow(warnings)]
mod bindings;
use bindings::{Guest, Input};
use example_helpers::{query_trigger, trigger::encode_trigger_output};

struct Component;

impl Guest for Component {
    fn run(input: Input) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
            let (trigger_id, req): (u64, Vec<u8>) =
                query_trigger!(Vec<u8>, &input, reactor.clone()).await?;
            anyhow::Ok(encode_trigger_output(trigger_id, req))
        })
        .map_err(|e| e.to_string())
    }
}

bindings::export!(Component with_types_in bindings);
