use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::pin::Pin;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use url::Url;

use crate::subsystems::trigger::error::TriggerError;
use crate::subsystems::trigger::streams::StreamTriggers;
use utils::telemetry::TriggerMetrics;
use wavs_types::AtProtoAction;

/// Configuration for ATProto Jetstream connection
#[derive(Debug, Clone)]
pub struct JetstreamConfig {
    /// Jetstream WebSocket endpoint URL
    pub endpoint: String,
    /// Collections to subscribe to. Empty vector means subscribe to all collections.
    /// Filtering is done in the lookup system based on registered triggers.
    pub wanted_collections: Vec<String>,
    /// Optional DIDs to filter for. None means listen to all repos.
    pub wanted_dids: Option<Vec<String>>,
    /// Cursor position for resuming (unix microseconds)
    pub cursor: Option<i64>,
    /// Compression enabled
    pub compression: bool,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Require hello message before starting stream
    pub require_hello: bool,
}

/// ATProto Jetstream event (source: https://github.com/bluesky-social/jetstream/blob/main/pkg/models/models.go)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JetstreamEvent {
    /// DID of the repository (can be at top level or in repo field)
    #[serde(alias = "did", alias = "repo")]
    pub did: String,
    /// Timestamp in microseconds
    #[serde(alias = "time_us", alias = "timeUs")]
    pub time_us: i64,
    /// Event kind (optional)
    #[serde(
        alias = "kind",
        alias = "type",
        skip_serializing_if = "Option::is_none"
    )]
    pub kind: Option<String>,
    /// Sequence number
    #[serde(alias = "seq", skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
    /// Commit data (present for commit events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<CommitData>,
    /// Account data (present for account events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountData>,
    /// Identity data (present for identity events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentityData>,
}

/// Commit data (source: https://github.com/bluesky-social/jetstream/blob/main/pkg/models/models.go)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<OperationField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "rkey")]
    pub rkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    /// Non-standard extension to support multi-op payloads seen in the wild.
    #[serde(skip_serializing_if = "Option::is_none", alias = "ops")]
    pub operations: Option<Vec<OperationData>>,
    /// Optional action used alongside nested operation objects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<AtProtoAction>,
}

/// Operation field can be a string action or an operation object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OperationField {
    Action(AtProtoAction),
    Detail(OperationData),
}

/// Operation data within commits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<AtProtoAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<Value>,
}

/// Account data (source: com.atproto.sync.subscribeRepos account message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountData {
    pub active: bool,
    pub did: String,
    pub seq: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub time: String,
}

/// Identity data (source: com.atproto.sync.subscribeRepos identity message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityData {
    pub did: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    pub seq: i64,
    pub time: String,
}

/// Subscription message for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriberMessage {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(rename = "wantedCollections")]
    wanted_collections: Vec<String>,
    #[serde(rename = "wantedDids")]
    wanted_dids: Option<Vec<String>>,
    cursor: Option<i64>,
}

/// Parsed ATProto event for internal use
#[derive(Debug, Clone)]
pub struct AtProtoEvent {
    /// Sequence number
    pub sequence: i64,
    /// Timestamp in microseconds
    pub timestamp: i64,
    /// Repository DID
    pub repo: String,
    /// Collection (NSID)
    pub collection: String,
    /// Record key
    pub rkey: String,
    /// Action type
    pub action: AtProtoAction,
    /// CID of the record
    pub cid: Option<String>,
    /// Record data (as JSON)
    pub record: Option<serde_json::Value>,
    /// Repository revision identifier for this commit (if provided)
    pub rev: Option<String>,
    /// Index of the operation within the commit (0-based)
    pub op_index: Option<u32>,
}

/// Create a Jetstream stream for ATProto events
pub async fn start_jetstream_stream(
    config: JetstreamConfig,
    metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let stream = async_stream::stream! {
        let mut reconnect_count = 0;
        let max_reconnects = 10;
        let base_delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(60);

        loop {
            info!("Connecting to Jetstream at: {}", config.endpoint);

            match create_jetstream_connection(&config, &metrics).await {
                Ok(mut stream) => {
                    reconnect_count = 0;
                    info!("Successfully connected to Jetstream");

                    while let Some(events) = stream.next().await {
                        match events {
                            Ok(atproto_events) => {
                                for atproto_event in atproto_events {
                                    yield Ok(StreamTriggers::AtProto {
                                        event: atproto_event,
                                    });
                                }
                            }
                            Err(TriggerError::JetstreamParse(msg)) => {
                                // Non-fatal parse issue (e.g. hello/keepalive)
                                warn!("Ignoring Jetstream message: {}", msg);
                                continue;
                            }
                            Err(e) => {
                                error!("Error processing Jetstream event: {:?}", e);
                                metrics.increment_total_errors("jetstream_event");
                                break; // Break from inner loop to trigger reconnect
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Jetstream connection error: {:?}", e);
                    metrics.increment_total_errors("jetstream_connection");

                    if reconnect_count >= max_reconnects {
                        error!("Max reconnection attempts reached, giving up");
                        yield Err(TriggerError::JetstreamConnection("Max reconnection attempts reached".to_string()));
                        return;
                    }

                    // Exponential backoff with jitter
                    let delay = std::cmp::min(
                        base_delay * 2_u32.pow(reconnect_count),
                        max_delay
                    ) + Duration::from_millis(rand::random::<u64>() % 1000);

                    warn!("Reconnecting in {:?} (attempt {})", delay, reconnect_count + 1);
                    sleep(delay).await;
                    reconnect_count += 1;
                }
            }
        }
    };

    Ok(Box::pin(stream))
}

/// Create a new Jetstream WebSocket connection
async fn create_jetstream_connection(
    config: &JetstreamConfig,
    _metrics: &TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<Vec<AtProtoEvent>, TriggerError>> + Send>>, TriggerError>
{
    let url = build_jetstream_url(config)?;
    info!("Connecting to Jetstream URL: {}", url);

    let (ws_stream, response) = connect_async(url.as_str()).await.map_err(|e| {
        TriggerError::JetstreamConnection(format!("WebSocket connection failed: {}", e))
    })?;

    info!("Jetstream connection established: {:?}", response.status());

    let ws_stream = ws_stream.map(|msg| match msg {
        Ok(Message::Text(text)) => handle_message(&text),
        Ok(Message::Binary(_)) => {
            warn!("Received binary message but compression is disabled");
            Err(TriggerError::JetstreamConnection(
                "Binary message received with compression disabled".to_string(),
            ))
        }
        Ok(Message::Close(_)) => {
            info!("Jetstream connection closed gracefully");
            Err(TriggerError::JetstreamConnection(
                "Connection closed".to_string(),
            ))
        }
        Err(e) => {
            error!("WebSocket error: {:?}", e);
            Err(TriggerError::JetstreamConnection(format!(
                "WebSocket error: {}",
                e
            )))
        }
        msg => {
            warn!("Unexpected message type: {:?}", msg);
            Err(TriggerError::JetstreamConnection(
                "Unexpected message type".to_string(),
            ))
        }
    });

    Ok(Box::pin(ws_stream))
}

/// Build Jetstream URL with query parameters
fn build_jetstream_url(config: &JetstreamConfig) -> Result<Url, TriggerError> {
    let mut url = Url::parse(&config.endpoint)
        .map_err(|e| TriggerError::JetstreamConfig(format!("Invalid endpoint URL: {}", e)))?;

    // Add query parameters
    if !config.wanted_collections.is_empty() {
        for collection in &config.wanted_collections {
            url.query_pairs_mut()
                .append_pair("wantedCollections", collection);
        }
    }

    if let Some(dids) = &config.wanted_dids {
        for did in dids {
            url.query_pairs_mut().append_pair("wantedDids", did);
        }
    }

    if let Some(cursor) = config.cursor {
        url.query_pairs_mut()
            .append_pair("cursor", &cursor.to_string());
    }

    if config.compression {
        url.query_pairs_mut().append_pair("compression", "zstd");
    }

    Ok(url)
}

/// Handle incoming Jetstream message
fn handle_message(text: &str) -> Result<Vec<AtProtoEvent>, TriggerError> {
    // Try to parse as subscriber message first
    if let Ok(_sub_msg) = serde_json::from_str::<SubscriberMessage>(text) {
        return Err(TriggerError::JetstreamParse(format!(
            "Subscriber message received; payload={}",
            text
        )));
    }

    let event = serde_json::from_str::<JetstreamEvent>(text).map_err(|e| {
        TriggerError::JetstreamParse(format!(
            "Failed to parse Jetstream event: {}; payload={}",
            e, text
        ))
    })?;

    let sequence = event.seq.unwrap_or(0);
    let timestamp = event.time_us;
    let did = event.did;

    if let Some(commit) = event.commit {
        parse_commit_event_typed(sequence, timestamp, did, commit)
    } else if let Some(identity) = event.identity {
        parse_identity_event_typed(sequence, timestamp, identity)
    } else if let Some(account) = event.account {
        parse_account_event_typed(sequence, timestamp, account)
    } else {
        Err(TriggerError::JetstreamParse(format!(
            "Unknown Jetstream event type; payload={}",
            text
        )))
    }
}

fn parse_commit_event_typed(
    sequence: i64,
    timestamp: i64,
    repo: String,
    commit: CommitData,
) -> Result<Vec<AtProtoEvent>, TriggerError> {
    // Prefer multi-operation formats when present
    if let Some(ops) = commit.operations.as_ref() {
        let mut events = Vec::with_capacity(ops.len());
        for (op_index, op) in ops.iter().enumerate() {
            let path = op.path.as_deref().ok_or_else(|| {
                TriggerError::JetstreamParse("Missing path in operation".to_string())
            })?;

            let (collection, rkey) = path.split_once('/').ok_or_else(|| {
                TriggerError::JetstreamParse(format!("Invalid path format: {}", path))
            })?;

            let action = op.action.clone().unwrap_or(AtProtoAction::Update);

            events.push(AtProtoEvent {
                sequence,
                timestamp,
                repo: repo.clone(),
                collection: collection.to_string(),
                rkey: rkey.to_string(),
                action,
                cid: op.cid.clone(),
                record: op.record.clone(),
                rev: commit.rev.clone(),
                op_index: Some(op_index as u32),
            });
        }
        return Ok(events);
    }

    if let Some(OperationField::Detail(op)) = &commit.operation {
        // Nested operation object
        let path = op
            .path
            .as_deref()
            .ok_or_else(|| TriggerError::JetstreamParse("Missing path in operation".to_string()))?;

        let (collection, rkey) = path.split_once('/').ok_or_else(|| {
            TriggerError::JetstreamParse(format!("Invalid path format: {}", path))
        })?;

        let action = commit
            .action
            .clone()
            .or_else(|| op.action.clone())
            .unwrap_or(AtProtoAction::Update);

        return Ok(vec![AtProtoEvent {
            sequence,
            timestamp,
            repo,
            collection: collection.to_string(),
            rkey: rkey.to_string(),
            action,
            cid: op.cid.clone().or_else(|| commit.cid.clone()),
            record: op.record.clone().or_else(|| commit.record.clone()),
            rev: commit.rev.clone(),
            op_index: None,
        }]);
    }

    // Single-operation commit shape
    let collection = commit
        .collection
        .as_deref()
        .ok_or_else(|| TriggerError::JetstreamParse("Missing collection in commit".to_string()))?;
    let rkey = commit
        .rkey
        .as_deref()
        .ok_or_else(|| TriggerError::JetstreamParse("Missing rkey in commit".to_string()))?;

    let action = match &commit.operation {
        Some(OperationField::Action(op)) => op.clone(),
        Some(OperationField::Detail(_)) => AtProtoAction::Update,
        None => AtProtoAction::Update,
    };

    Ok(vec![AtProtoEvent {
        sequence,
        timestamp,
        repo,
        collection: collection.to_string(),
        rkey: rkey.to_string(),
        action,
        cid: commit.cid.clone(),
        record: commit.record.clone(),
        rev: commit.rev.clone(),
        op_index: None,
    }])
}

fn parse_identity_event_typed(
    sequence: i64,
    timestamp: i64,
    identity: IdentityData,
) -> Result<Vec<AtProtoEvent>, TriggerError> {
    Ok(vec![AtProtoEvent {
        sequence,
        timestamp,
        repo: identity.did.clone(),
        collection: "identity".to_string(),
        rkey: "handle".to_string(),
        action: AtProtoAction::Update,
        cid: None,
        record: Some(json!({
            "handle": identity.handle,
            "seq": identity.seq,
            "time": identity.time
        })),
        rev: None,
        op_index: None,
    }])
}

fn parse_account_event_typed(
    sequence: i64,
    timestamp: i64,
    account: AccountData,
) -> Result<Vec<AtProtoEvent>, TriggerError> {
    Ok(vec![AtProtoEvent {
        sequence,
        timestamp,
        repo: account.did.clone(),
        collection: "account".to_string(),
        rkey: "status".to_string(),
        action: if account.active {
            AtProtoAction::Update
        } else {
            AtProtoAction::Delete
        },
        cid: None,
        record: Some(json!({
            "active": account.active,
            "status": account.status,
            "seq": account.seq,
            "time": account.time
        })),
        rev: None,
        op_index: None,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_jetstream_url() {
        let config = JetstreamConfig {
            endpoint: "wss://jetstream.example.com/subscribe".to_string(),
            wanted_collections: vec!["app.bsky.feed.post".to_string()],
            wanted_dids: Some(vec!["did:plc:test123".to_string()]),
            cursor: Some(12345),
            compression: true,
            max_message_size: 1024,
            require_hello: false,
        };

        let url = build_jetstream_url(&config).unwrap();
        assert_eq!(url.scheme(), "wss");
        assert_eq!(url.host_str().unwrap(), "jetstream.example.com");
        assert!(url
            .query_pairs()
            .any(|(k, v)| k == "wantedCollections" && v == "app.bsky.feed.post"));
        assert!(url
            .query_pairs()
            .any(|(k, v)| k == "wantedDids" && v == "did:plc:test123"));
        assert!(url
            .query_pairs()
            .any(|(k, v)| k == "cursor" && v == "12345"));
        assert!(url
            .query_pairs()
            .any(|(k, v)| k == "compression" && v == "zstd"));
    }

    #[test]
    fn test_handle_commit_message() {
        let commit_msg = r#"
        {
            "type": "commit",
            "seq": 12345,
            "timeUs": 1640995200000000,
            "repo": "did:plc:test123",
            "commit": {
                "seq": 12345,
                "rev": "testrev",
                "action": "create",
                "operation": {
                    "path": "app.bsky.feed.post/abcdef",
                    "cid": "bafytest123"
                }
            }
        }
        "#;

        let events = handle_message(commit_msg).unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.sequence, 12345);
        assert_eq!(event.repo, "did:plc:test123");
        assert_eq!(event.collection, "app.bsky.feed.post");
        assert_eq!(event.rkey, "abcdef");
        assert_eq!(event.action, AtProtoAction::Create);
        assert_eq!(event.cid, Some("bafytest123".to_string()));
    }

    #[test]
    fn test_handle_multiple_operations() {
        let commit_msg = r#"
        {
            "kind": "commit",
            "seq": 555,
            "timeUs": 1700000,
            "repo": "did:plc:multi",
            "commit": {
                "seq": 555,
                "rev": "multirev",
                "operations": [
                    {
                        "action": "create",
                        "path": "app.bsky.feed.post/aaa",
                        "cid": "cid-create-aaa",
                        "record": {"text": "hello"}
                    },
                    {
                        "action": "delete",
                        "path": "app.bsky.graph.follow/bbb",
                        "cid": "cid-delete-bbb"
                    }
                ]
            }
        }
        "#;

        let events = handle_message(commit_msg).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].collection, "app.bsky.feed.post");
        assert_eq!(events[0].rkey, "aaa");
        assert_eq!(events[0].action, AtProtoAction::Create);
        assert_eq!(events[0].cid.as_deref(), Some("cid-create-aaa"));
        assert!(events[0].record.is_some());

        assert_eq!(events[1].collection, "app.bsky.graph.follow");
        assert_eq!(events[1].rkey, "bbb");
        assert_eq!(events[1].action, AtProtoAction::Delete);
    }

    #[test]
    fn test_handle_ops_alias_and_delete_without_cid() {
        let commit_msg = r#"
        {
            "type": "commit",
            "seq": 777,
            "timeUs": 1700002,
            "repo": "did:plc:alias",
            "commit": {
                "ops": [
                    {
                        "action": "delete",
                        "path": "app.bsky.feed.post/zzz"
                    }
                ]
            }
        }
        "#;

        let events = handle_message(commit_msg).unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.collection, "app.bsky.feed.post");
        assert_eq!(event.rkey, "zzz");
        assert_eq!(event.action, AtProtoAction::Delete);
        assert!(event.cid.is_none());
    }

    #[test]
    fn test_handle_identity_event() {
        let identity_msg = r#"
        {
            "type": "identity",
            "seq": 901,
            "time_us": 1700003,
            "did": "did:plc:ident",
            "identity": {
                "did": "did:plc:ident",
                "handle": "example.bsky.social",
                "seq": 901,
                "time": "2024-01-01T00:00:00.000Z"
            }
        }
        "#;

        let events = handle_message(identity_msg).unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.collection, "identity");
        assert_eq!(event.action, AtProtoAction::Update);
        assert_eq!(event.repo, "did:plc:ident");
        assert!(event.record.is_some());
    }

    #[test]
    fn test_handle_account_event() {
        let account_msg = r#"
        {
            "type": "account",
            "seq": 902,
            "time_us": 1700004,
            "did": "did:plc:acct",
            "account": {
                "active": false,
                "did": "did:plc:acct",
                "seq": 902,
                "status": "takedown",
                "time": "2024-02-02T00:00:00.000Z"
            }
        }
        "#;

        let events = handle_message(account_msg).unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.collection, "account");
        assert_eq!(event.rkey, "status");
        assert_eq!(event.action, AtProtoAction::Delete);
        assert_eq!(event.repo, "did:plc:acct");
        assert!(event.record.is_some());
    }

    #[test]
    fn test_handle_commit_with_collection_rkey_fields() {
        let commit_msg = r#"
        {
            "kind": "commit",
            "seq": 888,
            "time_us": 1764955855798580,
            "did": "did:plc:u7kls5du676hvfr53pbrl7qc",
            "commit": {
                "cid": "bafyreiekmyvl7ogn4ym5lvligmc4xylntgvj7nu2rntseb7lfth6imdtyi",
                "collection": "app.bsky.feed.like",
                "operation": "create",
                "record": {
                    "$type": "app.bsky.feed.like",
                    "subject": {"cid": "bafyreifs5jhk7bssffjrqlzngnsdem3d7trc4cq2loormtio73ronscbr4", "uri": "at://did:plc:lm4bexmzwiwvcyp3xvnbqt3y/app.bsky.feed.post/3m7aqb43qqc2i"},
                    "createdAt": "2025-12-05T17:30:55.445Z"
                },
                "rev": "3m7azbh53l22h",
                "rkey": "3m7azbh4ous2h"
            }
        }
        "#;

        let events = handle_message(commit_msg).unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.collection, "app.bsky.feed.like");
        assert_eq!(event.rkey, "3m7azbh4ous2h");
        assert_eq!(event.action, AtProtoAction::Create);
        assert_eq!(
            event.cid.as_deref(),
            Some("bafyreiekmyvl7ogn4ym5lvligmc4xylntgvj7nu2rntseb7lfth6imdtyi")
        );
        assert!(event.record.is_some());
    }

    #[test]
    fn test_handle_invalid_path() {
        let commit_msg = r#"
        {
            "type": "commit",
            "seq": 111,
            "timeUs": 1700001,
            "repo": "did:plc:badpath",
            "commit": {
                "action": "create",
                "operation": {
                    "path": "missing-slash",
                    "cid": "cid1"
                }
            }
        }
        "#;

        let err = handle_message(commit_msg).unwrap_err();
        match err {
            TriggerError::JetstreamParse(msg) => {
                assert!(
                    msg.contains("Invalid commit.operation.path")
                        || msg.contains("Invalid path format")
                        || msg.contains("Missing path in operation")
                )
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
