//! KV store tools using wasi:keyvalue host bindings.
//!
//! KvGetTool — read a value from a named KV bucket by key.
//! KvSetTool — write a value to a named KV bucket.
//!
//! Both tools use the wasi:keyvalue/store interface provided by the WAVS WASI host.

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::Deserialize;

// Generate wasi:keyvalue bindings scoped to this module.
// The `imports` world in the keyvalue WIT exposes `store`, `atomics`, and `batch`.
// We only need `store` for basic get/set operations.
wit_bindgen::generate!({
    world: "imports",
    path: "../../wit-definitions/operator/wit/deps/wasi-keyvalue-0.2.0-draft2/package.wit",
    generate_all,
});

use wasi::keyvalue::store;

// ─── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum KvToolError {
    #[error("KV error: {0}")]
    KvError(String),

    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

// ─── KvGetTool ────────────────────────────────────────────────────────────────

/// Arguments for KvGetTool: bucket name and key to look up.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KvGetArgs {
    /// The KV bucket identifier (e.g., "agent-memory").
    pub bucket: String,
    /// The key to retrieve from the bucket.
    pub key: String,
}

/// Read a value from the WAVS KV store by bucket and key.
pub struct KvGetTool;

impl Tool for KvGetTool {
    const NAME: &'static str = "kv_get";

    type Error = KvToolError;
    type Args = KvGetArgs;
    type Output = Option<String>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read a value from WAVS KV store by bucket and key. Returns null if the key does not exist.".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(KvGetArgs))
                .unwrap_or(serde_json::Value::Null),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let bucket = store::open(&args.bucket)
            .map_err(|e| KvToolError::KvError(format!("{:?}", e)))?;

        let raw = bucket
            .get(&args.key)
            .map_err(|e| KvToolError::KvError(format!("{:?}", e)))?;

        match raw {
            Some(bytes) => {
                let s = String::from_utf8(bytes)?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }
}

// ─── KvSetTool ────────────────────────────────────────────────────────────────

/// Arguments for KvSetTool: bucket name, key, and value to store.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KvSetArgs {
    /// The KV bucket identifier (e.g., "agent-memory").
    pub bucket: String,
    /// The key to write in the bucket.
    pub key: String,
    /// The UTF-8 string value to store.
    pub value: String,
}

/// Write a value to the WAVS KV store.
pub struct KvSetTool;

impl Tool for KvSetTool {
    const NAME: &'static str = "kv_set";

    type Error = KvToolError;
    type Args = KvSetArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Write a value to WAVS KV store. Overwrites existing value if key exists.".to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(KvSetArgs))
                .unwrap_or(serde_json::Value::Null),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let bucket = store::open(&args.bucket)
            .map_err(|e| KvToolError::KvError(format!("{:?}", e)))?;

        bucket
            .set(&args.key, args.value.as_bytes())
            .map_err(|e| KvToolError::KvError(format!("{:?}", e)))?;

        Ok("ok".to_string())
    }
}
