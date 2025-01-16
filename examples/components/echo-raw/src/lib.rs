use layer_wasi::{
    bindings::{
        compat::TriggerData,
        world::{Guest, TriggerAction},
    },
    export_layer_trigger_world,
};
struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<u8>, String> {
        match trigger_action.data {
            TriggerData::Raw(data) => Ok(data),
            _ => Err("expected raw trigger data".to_string()),
        }
    }
}

export_layer_trigger_world!(Component);
