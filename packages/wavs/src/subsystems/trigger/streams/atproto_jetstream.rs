use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};
use url::Url;

use crate::subsystems::trigger::error::TriggerError;
use crate::subsystems::trigger::streams::StreamTriggers;
use utils::telemetry::TriggerMetrics;

/// Configuration for ATProto Jetstream connection
#[derive(Debug, Clone)]
pub struct JetstreamConfig {
    /// Jetstream WebSocket endpoint URL
    pub endpoint: String,
    /// Collections to subscribe to (e.g., "app.bsky.feed.post")
    pub wanted_collections: Vec<String>,
    /// Optional DIDs to filter for
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

impl Default for JetstreamConfig {
    fn default() -> Self {
        Self {
            endpoint: "wss://jetstream1.us-east.bsky.network/subscribe".to_string(),
            wanted_collections: vec!["app.bsky.feed.post".to_string()],
            wanted_dids: None,
            cursor: None,
            compression: true,
            max_message_size: 1024 * 1024, // 1MB
            require_hello: false,
        }
    }
}

/// ATProto Jetstream event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JetstreamEvent {
    /// Commit event (create/update/delete operations)
    Commit {
        #[serde(rename = "seq")]
        sequence: i64,
        #[serde(rename = "timeUs")]
        time_us: i64,
        repo: String,
        commit: CommitData,
    },
    /// Identity update event
    Identity {
        #[serde(rename = "seq")]
        sequence: i64,
        #[serde(rename = "timeUs")]
        time_us: i64,
        did: String,
        handle: Option<String>,
    },
    /// Account status event
    Account {
        #[serde(rename = "seq")]
        sequence: i64,
        #[serde(rename = "timeUs")]
        time_us: i64,
        did: String,
        active: bool,
        status: Option<String>,
    },
}

/// Commit data for Jetstream events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitData {
    #[serde(rename = "seq")]
    sequence: i64,
    rev: String,
    action: CommitAction,
    operation: Option<OperationData>,
}

/// Commit action type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CommitAction {
    Create,
    Update,
    Delete,
}

/// Operation data within commits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationData {
    path: String,
    cid: String,
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
    pub action: CommitAction,
    /// CID of the record
    pub cid: Option<String>,
    /// Record data (as JSON)
    pub record: Option<serde_json::Value>,
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

                    while let Some(event) = stream.next().await {
                        match event {
                            Ok(atproto_event) => {
                                yield Ok(StreamTriggers::AtProto {
                                    event: atproto_event,
                                });
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
) -> Result<Pin<Box<dyn Stream<Item = Result<AtProtoEvent, TriggerError>> + Send>>, TriggerError> {
    let url = build_jetstream_url(config)?;
    info!("Connecting to Jetstream URL: {}", url);

    let (ws_stream, response) = connect_async(url.as_str()).await.map_err(|e| {
        TriggerError::JetstreamConnection(format!("WebSocket connection failed: {}", e))
    })?;

    info!("Jetstream connection established: {:?}", response.status());

    let ws_stream = ws_stream.map(|msg| match msg {
        Ok(Message::Text(text)) => handle_message(&text, false),
        Ok(Message::Binary(data)) => {
            // Handle compressed messages
            match decompress_message(&data) {
                Ok(text) => handle_message(&text, true),
                Err(e) => {
                    error!("Failed to decompress message: {:?}", e);
                    Err(TriggerError::JetstreamDecompression(format!(
                        "Decompression failed: {}",
                        e
                    )))
                }
            }
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
fn handle_message(text: &str, _is_compressed: bool) -> Result<AtProtoEvent, TriggerError> {
    // Try to parse as subscriber message first
    if let Ok(_sub_msg) = serde_json::from_str::<SubscriberMessage>(text) {
        // This is a subscriber message, ignore for now
        return Err(TriggerError::JetstreamParse(
            "Subscriber message received".to_string(),
        ));
    }

    // Parse as Jetstream event
    let jetstream_event: JetstreamEvent = serde_json::from_str(text).map_err(|e| {
        TriggerError::JetstreamParse(format!("Failed to parse Jetstream event: {}", e))
    })?;

    match jetstream_event {
        JetstreamEvent::Commit {
            sequence,
            time_us: timestamp,
            repo,
            commit,
        } => {
            // Extract collection and rkey from the operation path
            let (collection, rkey) = if let Some(operation) = &commit.operation {
                let parts: Vec<&str> = operation.path.split('/').collect();
                if parts.len() == 3 {
                    (parts[1].to_string(), parts[2].to_string())
                } else {
                    return Err(TriggerError::JetstreamParse(
                        "Invalid operation path format".to_string(),
                    ));
                }
            } else {
                return Err(TriggerError::JetstreamParse(
                    "Missing operation data".to_string(),
                ));
            };

            let cid = commit.operation.as_ref().map(|op| op.cid.clone());

            // For now, we don't extract the full record data - it would need to be fetched separately
            let record = None;

            Ok(AtProtoEvent {
                sequence,
                timestamp,
                repo,
                collection,
                rkey,
                action: commit.action,
                cid,
                record,
            })
        }
        JetstreamEvent::Identity {
            sequence,
            time_us: timestamp,
            did,
            ..
        } => {
            // Convert identity event to a standard format
            Ok(AtProtoEvent {
                sequence,
                timestamp,
                repo: did.clone(),
                collection: "identity".to_string(),
                rkey: "handle".to_string(),
                action: CommitAction::Update,
                cid: None,
                record: None,
            })
        }
        JetstreamEvent::Account {
            sequence,
            time_us: timestamp,
            did,
            active,
            ..
        } => {
            // Convert account event to a standard format
            Ok(AtProtoEvent {
                sequence,
                timestamp,
                repo: did.clone(),
                collection: "account".to_string(),
                rkey: "status".to_string(),
                action: if active {
                    CommitAction::Update
                } else {
                    CommitAction::Delete
                },
                cid: None,
                record: None,
            })
        }
    }
}

/// Decompress zstd compressed message
fn decompress_message(data: &[u8]) -> Result<String, anyhow::Error> {
    // For now, we'll return a simple decompression implementation
    // In a production environment, you'd want to use the proper zstd dictionary
    // from the ATProto repository

    // Try to decompress without dictionary first
    match zstd::decode_all(data) {
        Ok(decompressed) => Ok(String::from_utf8(decompressed)?),
        Err(_) => {
            // If that fails, we might need the dictionary
            // For now, return an error
            Err(anyhow::anyhow!(
                "Failed to decompress message (may need zstd dictionary)"
            ))
        }
    }
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

        let event = handle_message(commit_msg, false).unwrap();
        assert_eq!(event.sequence, 12345);
        assert_eq!(event.repo, "did:plc:test123");
        assert_eq!(event.collection, "app.bsky.feed.post");
        assert_eq!(event.rkey, "abcdef");
        assert_eq!(event.action, CommitAction::Create);
        assert_eq!(event.cid, Some("bafytest123".to_string()));
    }
}
