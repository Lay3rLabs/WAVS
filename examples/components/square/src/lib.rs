#[allow(warnings)]
mod bindings;
use bindings::{Guest, Input};
use example_helpers::{query_trigger, trigger::encode_trigger_output};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(input: Input) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
            let (trigger_id, req) = query_trigger!(Request, &input, reactor.clone()).await?;

            let Request { x } = req;

            let y = x * x;

            anyhow::Ok(encode_trigger_output(
                trigger_id,
                serde_json::to_vec(&Response { y })?,
            ))
        })
        .map_err(|e| e.to_string())
    }
}

bindings::export!(Component with_types_in bindings);

#[derive(Deserialize, Debug)]
pub struct Request {
    pub x: u64,
}

#[derive(Serialize, Debug)]
pub struct Response {
    pub y: u64,
}
