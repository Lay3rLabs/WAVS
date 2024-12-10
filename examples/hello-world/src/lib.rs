#[allow(warnings)]
mod bindings;

use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use anyhow::Context;
use bindings::{Guest, Output, TaskQueueInput};

struct Component;

#[derive(Debug, PartialEq, Eq, Clone, RlpEncodable, RlpDecodable)]
pub struct HelloWorldTaskRlp {
    pub name: String,
    pub created_block: u32,
}

impl Guest for Component {
    fn run_task(input: TaskQueueInput) -> Output {
        match HelloWorldTaskRlp::decode(&mut input.request.as_slice())
            .context("Failed to parse request")
        {
            Ok(response) => {
                let mut output = Vec::new();
                response.encode(&mut output);
                Ok(output)
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

bindings::export!(Component with_types_in bindings);
