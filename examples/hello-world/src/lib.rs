#[allow(warnings)]
mod bindings;

use anyhow::Context;
use bindings::{Guest, Output, TaskQueueInput};
use serde::{Deserialize, Serialize};

struct Component;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct HelloWorldPayload {
    pub name: String,
    pub created_block: u32,
}

impl Guest for Component {
    fn run_task(input: TaskQueueInput) -> Output {
        // TODO: don't use json in, rather abi / rlp
        let request = serde_json::from_slice::<HelloWorldPayload>(&input.request)
            .context("Failed to parse request").map_err(|e| e.to_string())?;
        let response = format!("Hello World, {request.name}");
        // TODO: don't use json out, rather abi /rlp
        serde_json::to_vec(&response).map_err(|x| x.to_string())
    }
}

bindings::export!(Component with_types_in bindings);
