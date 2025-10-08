use std::collections::HashSet;

use alloy_primitives::{Address, B256};
use serde::ser::{Serialize, SerializeMap, SerializeStruct, Serializer};
use slotmap::Key;

use super::id::{RpcId, RpcRequestKind};

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
                SubscribeParams::Logs { addresses, topics } => {
                    RpcRequestKind::SubscribeLogs { addresses, topics }
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
    pub fn logs(addresses: HashSet<Address>, topics: HashSet<B256>) -> Self {
        Self::subscribe(SubscribeParams::Logs { addresses, topics })
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
        addresses: HashSet<Address>,
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
            SubscribeParams::Logs { addresses, topics } => {
                let mut map = serializer.serialize_map(None)?;
                if !addresses.is_empty() {
                    match addresses.len() {
                        1 => {
                            // If there's only one address, serialize as a single address
                            let address = addresses.iter().next().unwrap();
                            map.serialize_entry("address", address)?;
                        }
                        _ => {
                            // Otherwise, serialize as an array of addresses
                            // address is always OR semantics (i.e. A OR B OR C), unlike topics (see below)
                            // for the sake of easy debugging, we sort the addresses before serializing
                            let mut addresses: Vec<&Address> = addresses.iter().collect();
                            addresses.sort();
                            map.serialize_entry("address", &addresses)?;
                        }
                    }
                }
                if !topics.is_empty() {
                    // for topics, one-dimensional array in the spec means "AND"
                    // i.e. A AND B AND C, so they must _all_ exist to match the filter
                    // However, within that array, we can have another array to mean "OR"
                    // i.e. [A, [B, C], D] means A AND (B OR C) AND D
                    // Therefore, since we always want only OR semantics, we wrap our topics
                    // in an extra array so we get [[A, B, C]] which means (A OR B OR C)

                    // for the sake of easier debugging, we sort the topics before serializing
                    let mut topics: Vec<&B256> = topics.iter().collect();
                    topics.sort();
                    let topics = vec![topics];
                    map.serialize_entry("topics", &topics)?;
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
            "0x1234567890abcdef1234567890abcdef12345678"
        );

        // Test Logs subscription with multiple address and topics
        let req = RpcRequest::logs(
            [
                address!("0x1234567890abcdef1234567890abcdef12345678"),
                address!("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd"),
            ]
            .into_iter()
            .collect(),
            [
                b256!("0x00000000000000000000000000000000000000000000000000000000cafebabe"),
                b256!("0x00000000000000000000000000000000000000000000000000000000deadbeef"),
            ]
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
            serde_json::json!(vec![
                "0x1234567890abcdef1234567890abcdef12345678",
                "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd"
            ])
        );

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
