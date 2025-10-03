// inbound.rs

use alloy_primitives::B256;
use alloy_rpc_types_eth::{Header, Log};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use slotmap::{Key, KeyData};

use crate::subsystems::trigger::clients::evm::rpc_types::id::RpcId;

/// Inbound JSON-RPC messages from an Ethereum node.
#[derive(Debug)]
pub enum RpcInbound {
    /// Regular response (to a request we sent)
    Response {
        id: RpcId,
        result: Result<RpcResponse, RpcError>,
    },

    /// Subscription event
    Subscription {
        id: String, // subscription ID
        result: Result<RpcSubscriptionEvent, RpcError>,
    },
}

#[derive(Debug)]
// These are not absolutely guaranteed to be what they represent,
// the id *must* be checked against the request that was sent
// to be sure.
pub enum RpcResponse {
    /// A successful subscription creation
    NewSubscription { subscription_id: String },
    /// Result of `eth_unsubscribe`
    UnsubscribeAck(bool),
    /// Any other result (e.g. block number, generic call)
    Other(Value),
}

#[derive(Debug)]
pub enum RpcSubscriptionEvent {
    NewHeads(Header),
    Logs(Log),
    NewPendingTransaction(B256),
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

// === Custom Deserialization, so the Rust side gets nice clean data structures ===

impl<'de> Deserialize<'de> for RpcInbound {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Value = Deserialize::deserialize(deserializer)?;

        // Case 1: Response (has "id")
        if let Some(id_val) = v.get("id") {
            let id: u64 =
                serde_json::from_value(id_val.clone()).map_err(serde::de::Error::custom)?;

            // SAFE: RpcId is a new_key_type, so its KeyData is just a u64 internally
            // and we only care about its conversion in-memory, so we don't need to serde it
            let id: RpcId = KeyData::from_ffi(id).into();

            if let Some(err_val) = v.get("error") {
                let err: RpcError =
                    serde_json::from_value(err_val.clone()).map_err(serde::de::Error::custom)?;
                return Ok(RpcInbound::Response {
                    id,
                    result: Err(err),
                });
            }

            if let Some(result_val) = v.get("result") {
                // Try known response types
                let result = if result_val.is_boolean() {
                    let b: bool = serde_json::from_value(result_val.clone())
                        .map_err(serde::de::Error::custom)?;
                    Ok(RpcResponse::UnsubscribeAck(b))
                } else if result_val.is_string() {
                    let s: String = serde_json::from_value(result_val.clone())
                        .map_err(serde::de::Error::custom)?;
                    Ok(RpcResponse::NewSubscription { subscription_id: s })
                } else {
                    Ok(RpcResponse::Other(result_val.clone()))
                };

                return Ok(RpcInbound::Response { id, result });
            }
        }

        // Case 2: Subscription (method == "eth_subscription")
        if let Some(method) = v.get("method") {
            if method == "eth_subscription" {
                let params = v
                    .get("params")
                    .ok_or_else(|| serde::de::Error::custom("missing params"))?;

                let id: String = serde_json::from_value(
                    params
                        .get("subscription")
                        .cloned()
                        .ok_or_else(|| serde::de::Error::custom("missing subscription id"))?,
                )
                .map_err(serde::de::Error::custom)?;

                if let Some(err_val) = params.get("error") {
                    let err: RpcError = serde_json::from_value(err_val.clone())
                        .map_err(serde::de::Error::custom)?;
                    return Ok(RpcInbound::Subscription {
                        id,
                        result: Err(err),
                    });
                }

                let result_val = params
                    .get("result")
                    .ok_or_else(|| serde::de::Error::custom("missing result"))?;

                // Try parsing as Header
                if let Ok(header) = serde_json::from_value::<Header>(result_val.clone()) {
                    return Ok(RpcInbound::Subscription {
                        id,
                        result: Ok(RpcSubscriptionEvent::NewHeads(header)),
                    });
                }

                // Try parsing as Log
                if let Ok(log) = serde_json::from_value::<Log>(result_val.clone()) {
                    return Ok(RpcInbound::Subscription {
                        id,
                        result: Ok(RpcSubscriptionEvent::Logs(log)),
                    });
                }

                // Try parsing as tx hash
                if let Ok(hash) = serde_json::from_value::<B256>(result_val.clone()) {
                    return Ok(RpcInbound::Subscription {
                        id,
                        result: Ok(RpcSubscriptionEvent::NewPendingTransaction(hash)),
                    });
                }

                return Err(serde::de::Error::custom(format!(
                    "unrecognized subscription result: {}",
                    result_val
                )));
            }
        }

        Err(serde::de::Error::custom("unrecognized RpcInbound shape"))
    }
}

// === Tests ===

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use alloy_primitives::Address;
    use serde_json::json;

    #[test]
    fn response_new_subscription() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":"0xsubid"}"#;
        let msg: RpcInbound = serde_json::from_str(json).unwrap();

        match msg {
            RpcInbound::Response { id, result } => match result.unwrap() {
                RpcResponse::NewSubscription { subscription_id } => {
                    assert_eq!(id, RpcId::from(KeyData::from_ffi(1)));
                    assert_eq!(subscription_id, "0xsubid");
                }
                _ => panic!("Expected NewSubscription"),
            },
            _ => panic!("Expected Response"),
        }
    }

    #[test]
    fn response_unsubscribe_ack() {
        let json = r#"{"jsonrpc":"2.0","id":2,"result":true}"#;
        let msg: RpcInbound = serde_json::from_str(json).unwrap();

        match msg {
            RpcInbound::Response { id, result } => {
                assert_eq!(id, RpcId::from(KeyData::from_ffi(2)));
                match result.unwrap() {
                    RpcResponse::UnsubscribeAck(success) => assert!(success),
                    _ => panic!("Expected UnsubscribeAck"),
                }
            }
            _ => panic!("Expected Response"),
        }
    }

    #[test]
    fn response_error() {
        let json = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32602,"message":"Invalid params","data":null}}"#;
        let msg: RpcInbound = serde_json::from_str(json).unwrap();

        match msg {
            RpcInbound::Response { id, result } => {
                assert_eq!(id, RpcId::from(KeyData::from_ffi(3)));
                let err = result.unwrap_err();
                assert_eq!(err.code, -32602);
                assert_eq!(err.message, "Invalid params");
            }
            _ => panic!("Expected Response"),
        }
    }

    #[test]
    fn subscription_new_heads() {
        let json = r#"
            {
              "jsonrpc": "2.0",
              "method": "eth_subscription",
              "params": {
                "subscription": "0x6f356f57664f6d7949435244324c4c77",
                "result": {
                  "hash": "0x538e8a48c1ee547025c834c1294e742139d4eebfc664ecf11ae9c6bccd012d43",
                  "parentHash": "0x3c1f01fe7dd585a6303f9dc29d50fc30a7771da03835f9c841101f9dfdfd27fb",
                  "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                  "miner": "0x0000000000000000000000000000000000000000",
                  "stateRoot": "0x9c8eaf493f8b4edce2ba1647343eadcc0989cf461e712c0a6253ff2ca1842bb7",
                  "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                  "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                  "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                  "difficulty": "0x0",
                  "number": "0xb",
                  "gasLimit": "0x1c9c380",
                  "gasUsed": "0x0",
                  "timestamp": "0x68dec2e9",
                  "extraData": "0x",
                  "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "nonce": "0x0000000000000000",
                  "baseFeePerGas": "0xfae36fa",
                  "withdrawalsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                  "blobGasUsed": "0x0",
                  "excessBlobGas": "0x0",
                  "parentBeaconBlockRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                  "totalDifficulty": "0x0",
                  "size": "0x246",
                  "uncles": [],
                  "transactions": [],
                  "withdrawals": []
                }
              }
            }
        "#;

        let msg: RpcInbound = serde_json::from_str(json).unwrap();
        match msg {
            RpcInbound::Subscription { id, result } => {
                assert_eq!(id, "0x6f356f57664f6d7949435244324c4c77");
                match result.unwrap() {
                    RpcSubscriptionEvent::NewHeads(header) => {
                        assert_eq!(header.number, 11); // 0xb
                    }
                    _ => panic!("Expected NewHeads"),
                }
            }
            _ => panic!("Expected Subscription"),
        }
    }

    #[test]
    fn subscription_logs() {
        let json = r#"
            {
              "jsonrpc": "2.0",
              "method": "eth_subscription",
              "params": {
                "subscription": "0xabcd1234ef567890abcd1234ef567890",
                "result": {
                  "address": "0x1234567890abcdef1234567890abcdef12345678",
                  "blockHash": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                  "blockNumber": "0x1b4",
                  "data": "0x0000000000000000000000000000000000000000000000000000000000000042",
                  "logIndex": "0x0",
                  "removed": false,
                  "topics": [
                    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                    "0x0000000000000000000000001111111111111111111111111111111111111111",
                    "0x0000000000000000000000002222222222222222222222222222222222222222"
                  ],
                  "transactionHash": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                  "transactionIndex": "0x1"
                }
              }
            }
        "#;

        let msg: RpcInbound = serde_json::from_str(&json).unwrap();
        match msg {
            RpcInbound::Subscription { id, result } => {
                assert_eq!(id, "0xabcd1234ef567890abcd1234ef567890");
                match result.unwrap() {
                    RpcSubscriptionEvent::Logs(log) => {
                        assert_eq!(
                            log.address(),
                            Address::from_str("0x1234567890abcdef1234567890abcdef12345678")
                                .unwrap(),
                        );
                    }
                    _ => panic!("Expected Logs"),
                }
            }
            _ => panic!("Expected Subscription"),
        }
    }

    #[test]
    fn subscription_pending_tx() {
        let json = r#"{
            "jsonrpc":"2.0",
            "method":"eth_subscription",
            "params":{
                "subscription":"0xsubid",
                "result":"0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            }
        }"#;

        let msg: RpcInbound = serde_json::from_str(json).unwrap();
        match msg {
            RpcInbound::Subscription { id, result } => {
                assert_eq!(id, "0xsubid");
                match result.unwrap() {
                    RpcSubscriptionEvent::NewPendingTransaction(hash) => {
                        assert_eq!(
                            hash,
                            B256::from_slice(
                                &const_hex::decode(
                                    "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                                )
                                .unwrap()
                            )
                        );
                    }
                    _ => panic!("Expected NewPendingTransaction"),
                }
            }
            _ => panic!("Expected Subscription"),
        }
    }
}
