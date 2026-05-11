use example_helpers::{
    bindings::world::{
        exports::wavs::operator::agent::Guest as GuestAgent,
        wavs::operator::{
            input::TriggerAction,
            output::{StepResult, WasmResponse},
        },
        wasi::keyvalue::store,
        Guest,
    },
    export_layer_agent_world,
};

/// Multi-step continuation agent demonstrating the Continue/Done loop with KV-persisted state.
///
/// This component:
/// 1. On each invocation, reads a counter from the `agent_state` KV bucket
/// 2. Writes a checkpoint key `checkpoint:{N}` with value `completed step {N}`
/// 3. Increments the counter and writes it back
/// 4. Returns `Continue` for steps 0..2, then `Done` with a JSON summary on step 3
///
/// The `run` export (required by `export_layer_agent_world!`) is a stub that returns an error
/// directing callers to use the `run-agent` interface instead.
struct Component;

impl Guest for Component {
    fn run(_trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        Err("use run-agent interface".into())
    }
}

impl GuestAgent for Component {
    fn run_agent(_trigger_action: TriggerAction) -> Result<StepResult, String> {
        // Open the component-managed state bucket (NOT the engine-owned wavs_agent_step bucket)
        let state_bucket =
            store::open("agent_state").map_err(|e| format!("open agent_state: {e}"))?;

        // Read the current step counter (missing = step 0)
        let step: u32 = match state_bucket.get("step_counter").map_err(|e| e.to_string())? {
            Some(bytes) => {
                let s = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                s.parse::<u32>().map_err(|e| e.to_string())?
            }
            None => 0,
        };

        // Write checkpoint for this step
        let checkpoint_key = format!("checkpoint:{step}");
        let checkpoint_value = format!("completed step {step}");
        state_bucket
            .set(&checkpoint_key, checkpoint_value.as_bytes())
            .map_err(|e| format!("write checkpoint:{step}: {e}"))?;

        // Increment and persist counter
        let next_step = step + 1;
        state_bucket
            .set("step_counter", next_step.to_string().as_bytes())
            .map_err(|e| format!("write step_counter: {e}"))?;

        // After 3 steps (0, 1, 2), return Done on step 3
        if step < 3 {
            Ok(StepResult::Continue(format!("step_{next_step}")))
        } else {
            // Collect all checkpoint messages into a JSON summary
            let mut checkpoints = Vec::new();
            for i in 0..=step {
                let key = format!("checkpoint:{i}");
                let val = state_bucket
                    .get(&key)
                    .map_err(|e| format!("read {key}: {e}"))?
                    .unwrap_or_else(|| b"(missing)".to_vec());
                let msg = String::from_utf8(val).unwrap_or_else(|_| "(invalid utf8)".into());
                checkpoints.push(format!("{key}: {msg}"));
            }
            let summary = serde_json::to_vec(&checkpoints)
                .map_err(|e| format!("serialize summary: {e}"))?;

            Ok(StepResult::Done(vec![WasmResponse {
                payload: summary,
                ordering: None,
                event_id_salt: None,
            }]))
        }
    }
}

export_layer_agent_world!(Component);
