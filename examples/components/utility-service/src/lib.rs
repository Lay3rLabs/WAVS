use example_helpers::{
    bindings::world::{
        wavs::operator::{input::TriggerAction, output::WasmResponse},
        Guest,
    },
    export_layer_trigger_world,
};

/// Utility service — a simple callee component that receives a raw payload and echoes it back
/// with a "utility-response: " prefix. Used to prove that service-to-service RPC calls work
/// end-to-end (E2E-05).
///
/// This component uses `export_layer_trigger_world!` (legacy run-only interface).
/// Its service.json sets `allowed_callers: "all"` so any service may call it via call-service.
struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        // Extract raw payload from the trigger data
        let payload_bytes = match trigger_action.data {
            example_helpers::bindings::world::wavs::operator::input::TriggerData::Raw(bytes) => {
                bytes
            }
            _ => return Err("utility-service: expected Raw trigger data".to_string()),
        };

        // Prepend "utility-response: " to prove the call happened
        let prefix = b"utility-response: ";
        let mut response = Vec::with_capacity(prefix.len() + payload_bytes.len());
        response.extend_from_slice(prefix);
        response.extend_from_slice(&payload_bytes);

        Ok(vec![WasmResponse {
            payload: response,
            ordering: None,
            event_id_salt: None,
        }])
    }
}

export_layer_trigger_world!(Component);
