use example_helpers::bindings::compat::TriggerData;
use example_helpers::bindings::world::{Guest, TriggerAction, WasmResponse};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::encode_trigger_output;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        // hardcoding this because our tests are mostly about event-based triggers
        // but this component is not event-based
        let trigger_id = 1337;

        if let TriggerData::BlockInterval(data) = trigger_action.data {
            let return_data = format!("block-interval-data-{}", data.block_height);
            Ok(Some(encode_trigger_output(
                trigger_id,
                return_data.as_bytes(),
            )))
        } else {
            Err("Invalid trigger data".to_string())
        }
    }
}

export_layer_trigger_world!(Component);
