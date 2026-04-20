use anyhow::Result;
use example_helpers::prelude::*;
use rig::client::completion::CompletionClient;
use rig::completion::Prompt;
use serde::Serialize;
use wavs_rig::{
    HttpPermission, WavsAgent,
    anthropic::build_client,
    check_http_permission, run_agent,
    tools::KvSetTool,
};

/// Structured output returned by the agent.
#[derive(Serialize)]
struct AgentResult {
    prompt: String,
    answer: String,
}

/// Agent that accepts a text prompt and answers it with Anthropic, using KvSetTool to store the answer.
struct ExampleAgent {
    api_key: String,
}

impl WavsAgent for ExampleAgent {
    type Output = AgentResult;

    async fn run(&self, trigger_data: Vec<u8>) -> Result<AgentResult> {
        let prompt = String::from_utf8(trigger_data)?;

        // Build Anthropic client using WasiHttpClient (reqwest is unavailable on WASM)
        let client = build_client(&self.api_key)?;

        let agent = client
            .agent("claude-3-5-haiku-latest")
            .preamble(
                "Answer the question concisely. \
                 Use kv_set to store your answer with key 'last_answer'.",
            )
            .tool(KvSetTool)
            .build();

        let answer = agent.prompt(&prompt).await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(AgentResult { prompt, answer })
    }
}

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {
        // 1. Validate HTTP permission before attempting any LLM calls
        let sw = host::get_service();
        let workflow = sw
            .service
            .workflows
            .into_iter()
            .find(|(id, _)| *id == sw.workflow_id)
            .map(|(_, w)| w)
            .ok_or_else(|| "workflow not found".to_string())?;

        use example_helpers::bindings::world::wavs::types::service::AllowedHostPermission;
        let perm = match workflow.component.permissions.allowed_http_hosts {
            AllowedHostPermission::All => HttpPermission::All,
            AllowedHostPermission::None => HttpPermission::None,
            AllowedHostPermission::Only(hosts) => HttpPermission::Only(hosts),
        };
        check_http_permission(&perm)?;

        // 2. Read API key from environment (never hardcode)
        let api_key = std::env::var("WAVS_ENV_ANTHROPIC_API_KEY")
            .map_err(|_| "WAVS_ENV_ANTHROPIC_API_KEY not set".to_string())?;

        // 3. Extract prompt bytes from Raw trigger (manual trigger)
        let prompt_bytes = match trigger_action.data {
            TriggerData::Raw(data) => data,
            _ => return Err("agent-example expects Raw trigger data with prompt text".into()),
        };

        // 4. Run the agent — run_agent is the sole block_on boundary
        let output = run_agent(&ExampleAgent { api_key }, prompt_bytes)?;

        Ok(vec![WasmResponse {
            payload: output,
            ordering: None,
            event_id_salt: None,
        }])
    }
}

export_layer_trigger_world!(Component);
