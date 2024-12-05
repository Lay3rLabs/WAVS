#[allow(warnings)]
mod bindings;

use anyhow::{Context, Result};
use bindings::{Guest, Output, TaskQueueInput};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run_task(input: TaskQueueInput) -> Output {
        match inner_run_task(input) {
            Ok(response) => serde_json::to_vec(&response).map_err(|x| x.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn inner_run_task(input: TaskQueueInput) -> Result<Response> {
    let req: Request = serde_json::from_slice(&input.request).context("Failed to parse request")?;

    Ok(Response {
        message: req.message,
    })
}

#[derive(Deserialize, Serialize)]
struct Request {
    pub message: String,
}

#[derive(Deserialize, Serialize)]
struct Response {
    pub message: String,
}

bindings::export!(Component with_types_in bindings);
