use serde::{Deserialize, Serialize};
use wasmtime::component::bindgen;

bindgen!({
  world: "hello-world-world",
  // with: {
  //     "wasi": wasmtime_wasi::bindings,
  //     "wasi:http@0.2.0": wasmtime_wasi_http::bindings::http,
  // },
  path: "../../wit",
  async: true,
});

// stop-gap measure until we have a more generic pipeline

#[derive(Serialize, Deserialize)]
struct TempResponse {
    message_hash: Vec<u8>,
    task_name: String,
    task_created_block: u32,
    task_index: u32,
}

impl From<Response> for TempResponse {
    fn from(resp: Response) -> Self {
        let Response {
            message_hash,
            task_name,
            task_created_block,
            task_index,
        } = resp;

        TempResponse {
            message_hash,
            task_name,
            task_created_block,
            task_index,
        }
    }
}

pub fn temp_serialize_hello_world_component_response(resp: Response) -> Vec<u8> {
    let temp_resp = TempResponse::from(resp);
    serde_json::to_vec(&temp_resp).unwrap()
}

pub fn temp_deserialize_hello_world_component_response(data: &[u8]) -> Response {
    let temp_resp: TempResponse = serde_json::from_slice(data).unwrap();

    Response {
        message_hash: temp_resp.message_hash,
        task_name: temp_resp.task_name,
        task_created_block: temp_resp.task_created_block,
        task_index: temp_resp.task_index,
    }
}
