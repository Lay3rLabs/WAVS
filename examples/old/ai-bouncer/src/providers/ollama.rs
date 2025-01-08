use anyhow::Result;
use serde::{Deserialize, Serialize};

use layer_wasi::{Reactor, Request, WasiPollable};
use serde_json::json;

use crate::{
    session::{Session, SessionMessage},
    TaskInput,
};

use super::Provider;

#[derive(Serialize, Debug)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaChatMessage>,
    pub stream: bool,
    pub options: serde_json::Value,
    pub tools: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<OllamaChatMessageToolCall>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaChatMessageToolCall {
    pub function: OllamaChatMessageToolCallFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaChatMessageToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum OllamaChatResponse {
    Error(OllamaChatErrorResponse),
    Success(OllamaChatSuccessResponse),
}

#[derive(Deserialize, Debug)]
pub struct OllamaChatErrorResponse {
    pub error: String,
}

#[derive(Deserialize, Debug)]
pub struct OllamaChatSuccessResponse {
    pub message: OllamaChatMessage,
}

impl From<&SessionMessage> for OllamaChatMessage {
    fn from(value: &SessionMessage) -> Self {
        Self {
            role: value.role.clone(),
            content: value.content.clone(),
            tool_calls: value.decision.map(|d| {
                vec![OllamaChatMessageToolCall {
                    function: OllamaChatMessageToolCallFunction {
                        name: "make_decision".to_string(),
                        arguments: json!({"decision": d}).as_object().unwrap().clone(),
                    },
                }]
            }),
        }
    }
}

pub struct OllamaProvider {}

impl Provider for OllamaProvider {
    const NAME: &'static str = "ollama";

    fn new() -> Result<Self, String> {
        Ok(Self {})
    }

    async fn process(&self, reactor: &Reactor, input: &TaskInput) -> Result<Session, String> {
        let mut session = Session::load(
            &input.session_id,
            input.address.as_deref(),
            "llama3-groq-tool-use",
        )?;

        session.validate_message_id(input.message_id)?;
        session.add_user_message(&input.message);

        let mut req = Request::post("http://host.docker.internal:11434/api/chat")?;
        req.json(&OllamaChatRequest {
            model: session.model.clone(),
            messages: session.messages.iter().map(|m| m.into()).collect(),
            options: json!({
                "seed": session.seed,
                "temperature": session.temperature,
            }),
            tools: session.tools.clone(),
            stream: false,
        })?;

        let res = reactor.send(req).await?;

        let raw = String::from_utf8(res.body.clone());
        dbg!("ollama response: {:#?}", &raw);

        if res.status != 200 {
            return Err(format!("unexpected status code: {}", res.status));
        }

        match res.json::<OllamaChatResponse>() {
            Ok(response) => match response {
                OllamaChatResponse::Error(e) => Err(e.error),
                OllamaChatResponse::Success(response) => {
                    session.add_ai_message(
                        &response.message.content,
                        response.message.tool_calls.map(|calls| {
                            calls.len() == 1
                                && calls[0].function.arguments["decision"].as_bool().unwrap()
                        }),
                    );

                    Ok(session)
                }
            },
            Err(e) => Err(format!("response parsing error ({e}). body: {raw:?}")),
        }
    }
}
