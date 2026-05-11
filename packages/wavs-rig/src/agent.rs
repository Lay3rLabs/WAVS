//! Agent entry-point trait and async shim for WASI components.
//!
//! The `run_agent` function is the SOLE `block_on` boundary. All async code
//! (rig agent loop, tool calls, memory ops, LLM API calls) runs inside it.
//! NEVER call `block_on` inside async code — it will deadlock.

use serde::Serialize;
use wstd::runtime::block_on;

/// Trait implemented by WAVS agent components.
///
/// # Example
///
/// ```ignore
/// struct MyAgent { /* config, tools, memory */ }
///
/// impl WavsAgent for MyAgent {
///     type Output = MyResult;
///     async fn run(&self, trigger_data: Vec<u8>) -> anyhow::Result<Self::Output> {
///         // Parse trigger, call LLM, use tools, return structured result
///         todo!()
///     }
/// }
/// ```
pub trait WavsAgent {
    /// The structured output type returned by this agent.
    type Output: Serialize;

    /// Execute the agent logic with the given trigger data.
    ///
    /// This runs inside `block_on` — use `.await` freely but NEVER
    /// call `wstd::runtime::block_on` inside this method.
    fn run(
        &self,
        trigger_data: Vec<u8>,
    ) -> impl std::future::Future<Output = anyhow::Result<Self::Output>> + '_;
}

/// Run an agent inside a single `wstd::runtime::block_on` executor boundary.
///
/// This is the bridge between WASI's synchronous `Guest::run` and rig's async agent loop.
/// Returns JSON-serialized output bytes on success, or a human-readable error string.
///
/// # Usage in a WASI component
///
/// ```ignore
/// impl Guest for Component {
///     fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
///         let agent = MyAgent::new(/* ... */);
///         let output_bytes = wavs_rig::agent::run_agent(&agent, trigger_action.data)?;
///         // ... encode_trigger_output(trigger_id, output_bytes, ...)
///     }
/// }
/// ```
pub fn run_agent<A: WavsAgent>(agent: &A, trigger_data: Vec<u8>) -> Result<Vec<u8>, String> {
    block_on(async {
        let output = agent
            .run(trigger_data)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_vec(&output).map_err(|e| e.to_string())
    })
}
