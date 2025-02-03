use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{Guest, TriggerAction},
    export_layer_trigger_world,
};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<u8>, String> {
        let (trigger_id, req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;
        let req: Request = serde_json::from_slice(&req).map_err(|e| e.to_string())?;
        let y = req.x * req.x;
        let resp = serde_json::to_vec(&Response { y }).map_err(|e| e.to_string())?;
        Ok(encode_trigger_output(trigger_id, resp))
    }
}

#[derive(Deserialize, Debug)]
pub struct Request {
    pub x: u64,
}

#[derive(Serialize, Debug)]
pub struct Response {
    pub y: u64,
}

export_layer_trigger_world!(Component);
