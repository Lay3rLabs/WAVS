//! KV-backed conversation memory for rig agents.
//!
//! Stores conversation history as JSON in wasi:keyvalue. Supports append,
//! retrieve, and automatic truncation when estimated token count exceeds budget.

use serde::{Deserialize, Serialize};

use crate::kv_bindings::wasi::keyvalue::store;

/// Default token budget (characters / 4 approximation).
pub const DEFAULT_TOKEN_BUDGET: usize = 4000;

/// Key prefix for conversation storage to avoid collision with app KV data.
const KEY_PREFIX: &str = "wavs_agent_memory:";

/// A conversation message stored in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// KV-backed conversation memory with token budget enforcement.
///
/// Stores the full conversation as a JSON-serialized `Vec<Message>` under
/// a single KV key with the `wavs_agent_memory:` prefix.
pub struct WavsMemory {
    bucket: String,
    conversation_id: String,
    token_budget: usize,
}

impl WavsMemory {
    /// Create a new memory store.
    ///
    /// - `bucket`: KV bucket name (e.g., "default")
    /// - `conversation_id`: unique ID for this conversation
    /// - `token_budget`: max estimated tokens before truncation (DEFAULT_TOKEN_BUDGET if None)
    pub fn new(
        bucket: impl Into<String>,
        conversation_id: impl Into<String>,
        token_budget: Option<usize>,
    ) -> Self {
        Self {
            bucket: bucket.into(),
            conversation_id: conversation_id.into(),
            token_budget: token_budget.unwrap_or(DEFAULT_TOKEN_BUDGET),
        }
    }

    fn kv_key(&self) -> String {
        format!("{}{}", KEY_PREFIX, self.conversation_id)
    }

    fn load(&self) -> anyhow::Result<Vec<Message>> {
        let bucket = store::open(&self.bucket)
            .map_err(|e| anyhow::anyhow!("KV bucket open error: {:?}", e))?;
        match bucket
            .get(&self.kv_key())
            .map_err(|e| anyhow::anyhow!("KV read error: {:?}", e))?
        {
            Some(bytes) => Ok(serde_json::from_slice(&bytes)?),
            None => Ok(vec![]),
        }
    }

    fn save(&self, messages: &[Message]) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(messages)?;
        let bucket = store::open(&self.bucket)
            .map_err(|e| anyhow::anyhow!("KV bucket open error: {:?}", e))?;
        bucket
            .set(&self.kv_key(), &bytes)
            .map_err(|e| anyhow::anyhow!("KV write error: {:?}", e))?;
        Ok(())
    }

    /// Estimate token count using char-count / 4 heuristic.
    fn estimate_tokens(messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| (m.role.len() + m.content.len()) / 4)
            .sum()
    }

    /// Append a message and truncate oldest if over token budget.
    pub fn append(&self, message: Message) -> anyhow::Result<()> {
        let mut messages = self.load()?;
        messages.push(message);
        // Truncate oldest messages (keep at least 1) until within budget
        while Self::estimate_tokens(&messages) > self.token_budget && messages.len() > 1 {
            messages.remove(0);
        }
        self.save(&messages)
    }

    /// Retrieve full conversation history.
    pub fn retrieve(&self) -> anyhow::Result<Vec<Message>> {
        self.load()
    }

    /// Clear conversation history.
    pub fn clear(&self) -> anyhow::Result<()> {
        self.save(&[])
    }
}
