use std::sync::{atomic::AtomicUsize, Arc, LazyLock};

use serde::{Deserialize, Serialize};
use serde_json::json;

static RPC_REQUEST_ID: LazyLock<Arc<AtomicUsize>> = LazyLock::new(|| Arc::new(AtomicUsize::new(1)));

#[derive(Debug, Serialize)]
pub struct RpcRequest {
    jsonrpc: &'static str,
    id: String,
    method: &'static str,
    params: serde_json::Value,
}

impl RpcRequest {
    pub fn new(method: &'static str, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id: RPC_REQUEST_ID
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                .to_string(),
            method,
            params,
        }
    }

    pub fn blocks() -> Self {
        Self::new("eth_subscribe", json!(["newHeads"]))
    }

    pub fn unsubscribe(subscription_id: &str) -> Self {
        Self::new("eth_unsubscribe", json!([subscription_id]))
    }

    pub fn logs(filter: impl Serialize) -> Self {
        Self::new("eth_subscribe", json!(["logs", filter]))
    }
}

#[derive(Debug, Deserialize)]
pub struct RpcResponse<T> {
    jsonrpc: String,
    id: String,
    result: T,
}
