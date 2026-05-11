use example_helpers::{
    bindings::world::{
        exports::wavs::operator::agent::Guest as GuestAgent,
        host,
        wavs::operator::{
            input::TriggerAction,
            output::{StepResult, WasmResponse},
        },
        Guest,
    },
    export_layer_agent_world,
};

/// Composition agent — demonstrates service-to-service RPC by calling a utility service
/// via `call_service` and incorporating its response into the final result (E2E-05).
///
/// This component uses `export_layer_agent_world!` (full run + run-agent interface).
/// Its service.json sets `allowed_service_calls: "all"` so it may call any service.
///
/// The `run` export is a stub — this component is invoked through the agent interface.
/// The `run_agent` implementation:
/// 1. Reads the callee service ID from host config (key "callee_service_id")
/// 2. Forwards the trigger payload to the utility service via call_service
/// 3. Returns Done with the utility service's response as payload
struct Component;

impl Guest for Component {
    fn run(_trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        Err("use run-agent interface".into())
    }
}

impl GuestAgent for Component {
    fn run_agent(trigger_action: TriggerAction) -> Result<StepResult, String> {
        // Extract raw payload (the data to forward to the utility service)
        let payload = match trigger_action.data {
            example_helpers::bindings::world::wavs::operator::input::TriggerData::Raw(bytes) => {
                bytes
            }
            _ => {
                return Err(
                    "composition-agent: expected Raw trigger data containing callee_id:payload"
                        .to_string(),
                )
            }
        };

        // The callee service ID is passed via config var "callee_service_id"
        let callee_id = host::config_var("callee_service_id")
            .ok_or_else(|| "composition-agent: missing config var 'callee_service_id'".to_string())?;

        // Call the utility service with the raw payload
        let utility_response = host::call_service(&callee_id, &payload)
            .map_err(|e| format!("composition-agent: call_service failed: {}", e))?;

        // Build the final combined result:
        // "composition-result: <utility-service-response>"
        let prefix = b"composition-result: ";
        let mut combined = Vec::with_capacity(prefix.len() + utility_response.len());
        combined.extend_from_slice(prefix);
        combined.extend_from_slice(&utility_response);

        Ok(StepResult::Done(vec![WasmResponse {
            payload: combined,
            ordering: None,
            event_id_salt: None,
        }]))
    }
}

export_layer_agent_world!(Component);
