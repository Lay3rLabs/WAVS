use std::collections::HashSet;

use alloy_primitives::{Address, B256};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, SerializeStruct, Serializer};
use slotmap::Key;

use crate::subsystems::trigger::clients::evm::rpc_types::id::{RpcId, RpcRequestKind};

/// Outbound JSON-RPC request
///
/// Covers the supported `eth_subscribe` and `eth_unsubscribe` flows.
#[derive(Debug)]
pub enum RpcRequest {
    Subscribe { id: RpcId, params: SubscribeParams },
    Unsubscribe { id: RpcId, subscription_id: String },
}

impl RpcRequest {
    /// Return this request's ID.
    pub fn id(&self) -> RpcId {
        match self {
            RpcRequest::Subscribe { id, .. } => *id,
            RpcRequest::Unsubscribe { id, .. } => *id,
        }
    }

    /// Create a new subscribe request with auto-generated ID.
    pub fn subscribe(params: SubscribeParams) -> Self {
        Self::Subscribe {
            id: RpcId::new(match params.clone() {
                SubscribeParams::NewHeads => RpcRequestKind::SubscribeNewHeads,
                SubscribeParams::Logs { address, topics } => {
                    RpcRequestKind::SubscribeLogs { address, topics }
                }
                SubscribeParams::NewPendingTransactions => {
                    RpcRequestKind::SubscribeNewPendingTransactions
                }
            }),
            params,
        }
    }

    /// Subscribe to new block headers (`newHeads`).
    pub fn new_heads() -> Self {
        Self::subscribe(SubscribeParams::NewHeads)
    }

    /// Subscribe to new pending transactions (`newPendingTransactions`).
    pub fn new_pending_transactions() -> Self {
        Self::subscribe(SubscribeParams::NewPendingTransactions)
    }

    /// Subscribe to logs with an optional address/topic filter.
    pub fn logs(address: HashSet<Address>, topics: HashSet<B256>) -> Self {
        Self::subscribe(SubscribeParams::Logs { address, topics })
    }

    /// Create an unsubscribe request.
    pub fn unsubscribe(subscription_id: String) -> Self {
        Self::Unsubscribe {
            id: RpcId::new(RpcRequestKind::Unsubscribe {
                subscription_id: subscription_id.clone(),
            }),
            subscription_id,
        }
    }
}

/// Subscription parameters for `eth_subscribe`.
/// https://docs.metamask.io/services/reference/ethereum/json-rpc-methods/subscription-methods/eth_subscribe
#[derive(Debug, Clone)]
pub enum SubscribeParams {
    NewHeads,
    Logs {
        address: HashSet<Address>,
        topics: HashSet<B256>,
    },
    NewPendingTransactions,
}

// === Custom Serialization, so the Rust side gets nice clean data structures ===

impl Serialize for RpcRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("RpcRequest", 4)?;
        state.serialize_field("jsonrpc", "2.0")?;

        match self {
            RpcRequest::Subscribe { id, params } => {
                // SAFE: RpcId is a new_key_type, so its KeyData is just a u64 internally
                // and we only care about its conversion in-memory, so we don't need to serde it
                state.serialize_field("id", &id.data().as_ffi())?;
                state.serialize_field("method", "eth_subscribe")?;

                match params {
                    SubscribeParams::NewHeads | SubscribeParams::NewPendingTransactions => {
                        // Serialize as ["newHeads"] or ["newPendingTransactions"]
                        let params_array = vec![params];
                        state.serialize_field("params", &params_array)?;
                    }
                    SubscribeParams::Logs { .. } => {
                        // Serialize as ["logs", { filter }]
                        let params_array = ("logs", params);
                        state.serialize_field("params", &params_array)?;
                    }
                }
            }
            RpcRequest::Unsubscribe {
                id,
                subscription_id,
            } => {
                // SAFE: RpcId is a new_key_type, so its KeyData is just a u64 internally
                // and we only care about its conversion in-memory, so we don't need to serde it
                state.serialize_field("id", &id.data().as_ffi())?;
                state.serialize_field("method", "eth_unsubscribe")?;
                let params_array = vec![subscription_id];
                state.serialize_field("params", &params_array)?;
            }
        }

        state.end()
    }
}

impl Serialize for SubscribeParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SubscribeParams::NewHeads => serializer.serialize_str("newHeads"),
            SubscribeParams::NewPendingTransactions => {
                serializer.serialize_str("newPendingTransactions")
            }
            SubscribeParams::Logs { address, topics } => {
                let mut map = serializer.serialize_map(None)?;
                if !address.is_empty() {
                    map.serialize_entry("address", address)?;
                }
                if !topics.is_empty() {
                    // Each topic is an AND semantics, so we wrap in an extra array for
                    // OR semantics with extra embedding
                    let list = vec![topics];
                    map.serialize_entry("topics", &list)?;
                }
                map.end()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, b256};
    use serde_json::Value;

    #[test]
    fn rpc_request_serialization() {
        // Test NewHeads subscription
        let req = RpcRequest::new_heads();
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_subscribe");
        assert_eq!(parsed["params"], serde_json::json!(["newHeads"]));
        assert!(parsed["id"].is_number());

        // Test NewPendingTransactions subscription
        let req = RpcRequest::new_pending_transactions();
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_subscribe");
        assert_eq!(
            parsed["params"],
            serde_json::json!(["newPendingTransactions"])
        );
        assert!(parsed["id"].is_number());

        // Test Logs subscription with address and topics
        let req = RpcRequest::logs(
            [address!("0x1234567890abcdef1234567890abcdef12345678")]
                .into_iter()
                .collect(),
            [b256!(
                "0x00000000000000000000000000000000000000000000000000000000deadbeef"
            )]
            .into_iter()
            .collect(),
        );
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_subscribe");
        assert_eq!(parsed["params"][0], "logs");
        assert_eq!(
            parsed["params"][1]["address"],
            serde_json::json!(["0x1234567890abcdef1234567890abcdef12345678"])
        );
        // topics contains both a concrete B256 and a null entry
        assert_eq!(
            parsed["params"][1]["topics"],
            serde_json::json!(vec![[
                "0x00000000000000000000000000000000000000000000000000000000deadbeef"
            ]])
        );
        assert!(parsed["id"].is_number());

        // Test Logs subscription with no filters
        let req = RpcRequest::logs(HashSet::new(), HashSet::new());
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_subscribe");
        assert_eq!(parsed["params"][0], "logs");
        assert_eq!(parsed["params"][1], serde_json::json!({}));
        assert!(parsed["id"].is_number());

        // Test Unsubscribe
        let req = RpcRequest::unsubscribe("0x123abc".to_string());
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_unsubscribe");
        assert_eq!(parsed["params"], serde_json::json!(["0x123abc"]));
        assert!(parsed["id"].is_number());

        // Test that IDs are unique across requests
        let req1 = RpcRequest::new_heads();
        let req2 = RpcRequest::new_heads();
        let json1 = serde_json::to_string(&req1).unwrap();
        let json2 = serde_json::to_string(&req2).unwrap();
        let parsed1: Value = serde_json::from_str(&json1).unwrap();
        let parsed2: Value = serde_json::from_str(&json2).unwrap();
        assert_ne!(parsed1["id"], parsed2["id"]);

        // Type safety - ensure we can match on the enum variants
        match req1 {
            RpcRequest::Subscribe { id, params } => {
                assert!(matches!(id.kind(), Some(RpcRequestKind::SubscribeNewHeads)));
                match params {
                    SubscribeParams::NewHeads => { /* expected */ }
                    _ => panic!("Expected NewHeads params"),
                }
            }
            RpcRequest::Unsubscribe { .. } => panic!("Expected Subscribe variant"),
        }

        match RpcRequest::unsubscribe("test".to_string()) {
            RpcRequest::Unsubscribe {
                id,
                subscription_id,
            } => {
                match id.kind() {
                    Some(RpcRequestKind::Unsubscribe { subscription_id }) => {
                        assert_eq!(subscription_id, "test")
                    }
                    _ => panic!("Expected Unsubscribe kind"),
                }
                assert_eq!(subscription_id, "test");
            }
            RpcRequest::Subscribe { .. } => panic!("Expected Unsubscribe variant"),
        }
    }
}
