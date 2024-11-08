use anyhow::Result;
use serde::{Deserialize, Serialize};

use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionMessage {
    /// the role of the sender of this message.
    pub role: String,
    /// the content of the message.
    pub content: String,
    /// the decision made by this message. this must be from the AI.
    pub decision: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    /// the id of the session.
    pub id: String,
    /// the address of the user interacting with this session.
    pub address: String,
    /// the decision made by the AI in this session.
    pub decision: Option<bool>,
    /// the model to use for this session.
    pub model: String,
    /// the messages in this session.
    pub messages: Vec<SessionMessage>,
    /// the seed for this session.
    pub seed: u64,
    /// the temperature for this session.
    pub temperature: f32,
    /// the tools available to the AI in this session.
    pub tools: Vec<serde_json::Value>,
}

impl Session {
    pub fn path(session_id: &str) -> String {
        format!("./sessions/{}.json", session_id)
    }

    pub fn load(
        session_id: &str,
        address_if_new: Option<&str>,
        model_if_new: &str,
    ) -> Result<Self, String> {
        let path = Self::path(session_id);
        let path = std::path::Path::new(&path);

        let session = if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read session file: {e}"))?;

            serde_json::from_str(&content)
                .map_err(|e| format!("failed to parse session file: {e}"))?
        } else {
            std::fs::create_dir_all("./sessions")
                .map_err(|e| format!("failed to create sessions directory: {e}"))?;

            let address = address_if_new
                .as_ref()
                .ok_or("address is required on first message")?;

            Session {
                id: session_id.to_string(),
                address: address.to_string(),
                decision: None,
                model: model_if_new.to_string(),
                messages: vec![
                    SessionMessage {
                        role: "system".to_string(),
                        content: "You are a bouncer, deciding who is allowed to join an organization based purely on good vibes. Engage in a conversation with the user, and make a decision using the make_decision tool only once you are confident you have enough information. You should not make a decision until exchanging at least a few messages back and forth.".to_string(),
                        decision: None,
                    },
                ],
                seed: 42,
                temperature: 0.4,
                tools: vec![json!({
                    "type": "function",
                    "function": {
                        "name": "make_decision",
                        "description": "Make a decision about whether or not to allow the user into the organization.",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "decision": {
                                    "type": "boolean",
                                    "description": "Whether or not to allow the user into the organization."
                                }
                            },
                            "required": ["decision"]
                        }
                    }
                })],
            }
        };

        // ensure the session does not yet have a decision
        if session.decision.is_some() {
            return Err("a decision has already been made".to_string());
        }

        Ok(session)
    }

    /// ensure the message_id is the next user message
    pub fn validate_message_id(&self, message_id: u16) -> Result<(), String> {
        let user_messages = self.messages.iter().filter(|m| m.role == "user").count();

        if message_id != user_messages as u16 {
            return Err(format!(
                "message_id mismatch: expected {} but got {}",
                user_messages, message_id
            ));
        }

        Ok(())
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(SessionMessage {
            role: "user".to_string(),
            content: content.to_string(),
            decision: None,
        });
    }

    pub fn add_ai_message(&mut self, content: &str, decision: Option<bool>) {
        if let Some(decision) = decision {
            self.decision = Some(decision);
        }

        self.messages.push(SessionMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
            decision,
        });
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path(&self.id);
        std::fs::write(path, serde_json::to_string(&self).unwrap())
            .map_err(|e| format!("failed to write session file: {e}"))
    }
}
