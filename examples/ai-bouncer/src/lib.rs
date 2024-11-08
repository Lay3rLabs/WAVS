#[allow(warnings)]
mod bindings;

mod providers;
mod session;

use bindings::{Guest, Output, TaskQueueInput};
use layer_wasi::{block_on, Reactor};
use providers::Provider;
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run_task(request: TaskQueueInput) -> Output {
        let provider =
            std::env::var("PROVIDER").or(Err("missing env var `PROVIDER`".to_string()))?;

        let res = match serde_json::from_slice(&request.request) {
            Ok(input) => block_on(|reactor| get_output(reactor, provider, input)),
            Err(e) => serde_json::to_vec(&TaskOutput::Error(format!(
                "Could not deserialize input request from JSON: {}",
                e
            )))
            .map_err(|e| e.to_string()),
        };

        res
    }
}

async fn get_output(
    reactor: Reactor,
    provider: String,
    input: TaskInput,
) -> Result<Vec<u8>, String> {
    let session = match provider.as_str() {
        providers::ollama::OllamaProvider::NAME => {
            let provider = providers::ollama::OllamaProvider::new()?;

            provider.process(&reactor, &input).await
        }
        providers::groq::GroqProvider::NAME => {
            let provider = providers::groq::GroqProvider::new()?;

            provider.process(&reactor, &input).await
        }
        _ => Err(format!("unknown provider: {provider}")),
    };

    let output = session
        .and_then(|session| {
            session.save()?;

            Ok(TaskOutput::Success(TaskOutputSuccess {
                session_id: session.id,
                address: session.address,
                message_id: input.message_id,
                response: session.messages.last().unwrap().content.clone(),
                decision: session.messages.last().unwrap().decision,
            }))
        })
        .unwrap_or_else(TaskOutput::Error);

    serde_json::to_vec(&output).map_err(|e| e.to_string())
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    /// the session ID of the address being evaluated
    pub session_id: String,
    /// the address being evaluated. only needed on first message (where ID = 0)
    pub address: Option<String>,
    /// the incrementing message index, starting at 0
    pub message_id: u16,
    /// the next message in the conversation
    pub message: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum TaskOutput {
    Success(TaskOutputSuccess),
    Error(String),
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskOutputSuccess {
    /// the session ID of the address being evaluated
    pub session_id: String,
    /// the address being evaluated
    pub address: String,
    /// the message ID being responded to
    pub message_id: u16,
    /// the response to the message
    pub response: String,
    /// the decision made by the AI bouncer, which will be present once
    /// finalized
    pub decision: Option<bool>,
}

bindings::export!(Component with_types_in bindings);
