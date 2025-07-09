use example_helpers::bindings::world::{
    wavs::worker::{input::TriggerAction, output::WasmResponse},
    Guest,
};

use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};

use example_types::{SquareRequest, SquareResponse};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        let (trigger_id, req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;
        let req: SquareRequest = serde_json::from_slice(&req).map_err(|e| e.to_string())?;
        let y = req.x * req.x;
        let resp = serde_json::to_vec(&SquareResponse { y }).map_err(|e| e.to_string())?;
        Ok(Some(encode_trigger_output(trigger_id, resp)))
    }
}

export_layer_trigger_world!(Component);
