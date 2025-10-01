use std::sync::{atomic::AtomicUsize, Arc, LazyLock};

use serde::{ser::SerializeSeq, Deserialize, Serialize, Serializer};

static RPC_REQUEST_ID: LazyLock<Arc<AtomicUsize>> = LazyLock::new(|| Arc::new(AtomicUsize::new(1)));

#[derive(Debug)]
pub enum RpcRequest {
    Subscribe { id: usize, params: SubscribeParams },
    Unsubscribe { id: usize, subscription_id: String },
}

impl RpcRequest {
    pub fn id(&self) -> usize {
        match self {
            RpcRequest::Subscribe { id, .. } => *id,
            RpcRequest::Unsubscribe { id, .. } => *id,
        }
    }
}

#[derive(Debug)]
// https://docs.metamask.io/services/reference/ethereum/json-rpc-methods/subscription-methods/eth_subscribe
pub enum SubscribeParams {
    NewHeads,
    Logs {
        address: Option<Vec<String>>,
        topics: Option<Vec<String>>,
    },
    NewPendingTransactions,
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
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(None)?;
                if let Some(address) = address {
                    map.serialize_entry("address", address)?;
                }
                if let Some(topics) = topics {
                    map.serialize_entry("topics", topics)?;
                }
                map.end()
            }
        }
    }
}

impl Serialize for RpcRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("RpcRequest", 4)?;
        state.serialize_field("jsonrpc", "2.0")?;

        match self {
            RpcRequest::Subscribe { id, params } => {
                state.serialize_field("id", &id.to_string())?;
                state.serialize_field("method", "eth_subscribe")?;

                match params {
                    SubscribeParams::NewHeads | SubscribeParams::NewPendingTransactions => {
                        let params_array = vec![params];
                        state.serialize_field("params", &params_array)?;
                    }
                    SubscribeParams::Logs { .. } => {
                        // For logs, we need ["logs", filter_object]
                        struct LogsParams<'a>(&'a SubscribeParams);
                        impl<'a> Serialize for LogsParams<'a> {
                            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                            where
                                S: Serializer,
                            {
                                let mut seq = serializer.serialize_seq(Some(2))?;
                                seq.serialize_element("logs")?;
                                seq.serialize_element(self.0)?;
                                seq.end()
                            }
                        }
                        state.serialize_field("params", &LogsParams(params))?;
                    }
                }
            }
            RpcRequest::Unsubscribe {
                id,
                subscription_id,
            } => {
                state.serialize_field("id", &id.to_string())?;
                state.serialize_field("method", "eth_unsubscribe")?;
                let params_array = vec![subscription_id];
                state.serialize_field("params", &params_array)?;
            }
        }

        state.end()
    }
}

impl RpcRequest {
    pub fn subscribe(params: SubscribeParams) -> Self {
        Self::Subscribe {
            id: RPC_REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            params,
        }
    }

    pub fn new_heads() -> Self {
        Self::subscribe(SubscribeParams::NewHeads)
    }

    pub fn new_pending_transactions() -> Self {
        Self::subscribe(SubscribeParams::NewPendingTransactions)
    }

    pub fn logs(address: Option<Vec<String>>, topics: Option<Vec<String>>) -> Self {
        Self::subscribe(SubscribeParams::Logs { address, topics })
    }

    pub fn unsubscribe(subscription_id: String) -> Self {
        Self::Unsubscribe {
            id: RPC_REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            subscription_id,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: String,
    #[serde(flatten)]
    pub payload: RpcResponsePayload,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RpcResponsePayload {
    Success { result: RpcResult },
    Error { error: RpcError },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RpcResult {
    SubscriptionId(String),
    UnsubscribeSuccess(bool),
    SubscriptionData {
        subscription: String,
        result: serde_json::Value,
    },
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl RpcResponse {
    pub fn is_success(&self) -> bool {
        matches!(self.payload, RpcResponsePayload::Success { .. })
    }

    pub fn is_error(&self) -> bool {
        matches!(self.payload, RpcResponsePayload::Error { .. })
    }

    pub fn subscription_id(&self) -> Option<&str> {
        match &self.payload {
            RpcResponsePayload::Success {
                result: RpcResult::SubscriptionId(id),
            } => Some(id),
            _ => None,
        }
    }

    pub fn unsubscribe_success(&self) -> Option<bool> {
        match &self.payload {
            RpcResponsePayload::Success {
                result: RpcResult::UnsubscribeSuccess(success),
            } => Some(*success),
            _ => None,
        }
    }

    pub fn subscription_data(&self) -> Option<(&str, &serde_json::Value)> {
        match &self.payload {
            RpcResponsePayload::Success {
                result:
                    RpcResult::SubscriptionData {
                        subscription,
                        result,
                    },
            } => Some((subscription, result)),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<&RpcError> {
        match &self.payload {
            RpcResponsePayload::Error { error } => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn evm_rpc_request_serialization() {
        use super::*;
        use serde_json::Value;

        // Test NewHeads subscription
        let req = RpcRequest::new_heads();
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_subscribe");
        assert_eq!(parsed["params"], serde_json::json!(["newHeads"]));
        assert!(parsed["id"].is_string());

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
        assert!(parsed["id"].is_string());

        // Test Logs subscription with address and topics
        let req = RpcRequest::logs(
            Some(vec![
                "0x1234567890abcdef1234567890abcdef12345678".to_string()
            ]),
            Some(vec!["0xdeadbeef".to_string(), "0x54321".to_string()]),
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
        assert_eq!(
            parsed["params"][1]["topics"],
            serde_json::json!(["0xdeadbeef", "0x54321"])
        );
        assert!(parsed["id"].is_string());

        // Test Logs subscription with no filters
        let req = RpcRequest::logs(None, None);
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_subscribe");
        assert_eq!(parsed["params"][0], "logs");
        assert_eq!(parsed["params"][1], serde_json::json!({}));
        assert!(parsed["id"].is_string());

        // Test Unsubscribe
        let req = RpcRequest::unsubscribe("0x123abc".to_string());
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["method"], "eth_unsubscribe");
        assert_eq!(parsed["params"], serde_json::json!(["0x123abc"]));
        assert!(parsed["id"].is_string());

        // Test that IDs are unique across requests
        let req1 = RpcRequest::new_heads();
        let req2 = RpcRequest::new_heads();
        let json1 = serde_json::to_string(&req1).unwrap();
        let json2 = serde_json::to_string(&req2).unwrap();
        let parsed1: Value = serde_json::from_str(&json1).unwrap();
        let parsed2: Value = serde_json::from_str(&json2).unwrap();
        assert_ne!(parsed1["id"], parsed2["id"]);

        // Test type safety - ensure we can match on the enum variants
        match req1 {
            RpcRequest::Subscribe { id, params } => {
                assert!(id > 0);
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
                assert!(id > 0);
                assert_eq!(subscription_id, "test");
            }
            RpcRequest::Subscribe { .. } => panic!("Expected Unsubscribe variant"),
        }
    }

    #[test]
    fn evm_rpc_response_deserialization() {
        use super::*;

        // Test successful subscription response
        let json = r#"{"jsonrpc":"2.0","id":"1","result":"0x9cef478923ff08bf67fde6c64013158d"}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, "1");
        assert!(response.is_success());
        assert!(!response.is_error());
        assert_eq!(
            response.subscription_id(),
            Some("0x9cef478923ff08bf67fde6c64013158d")
        );

        // Test successful unsubscribe response
        let json = r#"{"jsonrpc":"2.0","id":"2","result":true}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();

        assert!(response.is_success());
        assert_eq!(response.unsubscribe_success(), Some(true));

        // Test subscription data notification
        let json = r#"{"jsonrpc":"2.0","id":"3","result":{"subscription":"0x9cef478923ff08bf67fde6c64013158d","result":{"number":"0x1b4","hash":"0x..."}}}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();

        assert!(response.is_success());
        let (subscription, data) = response.subscription_data().unwrap();
        assert_eq!(subscription, "0x9cef478923ff08bf67fde6c64013158d");
        assert_eq!(data["number"], "0x1b4");
        assert_eq!(data["hash"], "0x...");

        // Test error response
        let json = r#"{"jsonrpc":"2.0","id":"4","error":{"code":-32602,"message":"Invalid params","data":null}}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();

        assert!(!response.is_success());
        assert!(response.is_error());
        let error = response.error().unwrap();
        assert_eq!(error.code, -32602);
        assert_eq!(error.message, "Invalid params");
        assert!(error.data.is_none());

        // Test error response with data
        let json = r#"{"jsonrpc":"2.0","id":"5","error":{"code":-32000,"message":"Server error","data":{"details":"Connection failed"}}}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();

        let error = response.error().unwrap();
        assert_eq!(error.code, -32000);
        assert_eq!(error.message, "Server error");
        assert!(error.data.is_some());
        assert_eq!(error.data.as_ref().unwrap()["details"], "Connection failed");
    }
}
