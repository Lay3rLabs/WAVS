#[allow(warnings)]
mod bindings;

use anyhow::anyhow;
use bindings::{Error, Guest, Input, Output};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct RequestData {
    pub payload: TaskRequestData,
    pub id: u64,
    pub expires: u64,
}

#[derive(Deserialize, Debug)]
pub struct TaskRequestData {
    pub x: u64,
}

#[derive(Serialize, Debug)]
pub struct ResponseData {
    pub id: u64,
    pub response: TaskResponseData,
}

#[derive(Serialize, Debug)]
pub struct TaskResponseData {
    pub y: u64,
}
struct Component;

impl Guest for Component {
    fn run_task(request: Input) -> Result<Output, Error> {
        let RequestData { id, payload, .. } = serde_json::from_str(&request.request)
            .map_err(|e| anyhow!("Could not deserialize input request from JSON: {}", e))
            .unwrap();
        let y = payload.x * payload.x;
        println!("{}^2 = {}", payload.x, y);

        let response_data = ResponseData {
            id,
            response: TaskResponseData { y },
        };
        let response = serde_json::to_string(&response_data)
            .map_err(|e| anyhow!("Could not serialize response JSON: {}", e))
            .unwrap();

        Ok(Output { response })
    }
}

bindings::export!(Component with_types_in bindings);
