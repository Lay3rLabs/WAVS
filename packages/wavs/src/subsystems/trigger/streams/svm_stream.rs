use futures::{stream, Stream};
use std::collections::BTreeMap;
use std::pin::Pin;
use utils::{svm_client::SvmQueryClient, telemetry::TriggerMetrics};
use wavs_types::{SvmParsedEvent, ChainName};

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

/// Data structure representing a single SVM program log entry
#[derive(Debug, Clone)]
pub struct SvmProgramLog {
    pub program_id: String,
    pub logs: Vec<String>,
    pub success: bool,
}

pub async fn start_svm_event_stream(
    query_client: SvmQueryClient,
    chain_name: ChainName,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    use futures::stream::StreamExt;

    let chain_name_clone = chain_name.clone();

    let event_stream = async_stream::stream! {
        // Subscribe to all program logs (watch everything as requested)
        let client = match query_client.endpoint.to_pubsub_client().await {
            Ok(client) => client,
            Err(e) => {
                yield Err(TriggerError::SvmSubscription(e.into()));
                return;
            }
        };

        // Subscribe to all logs (not filtered by program ID)
        let logs_config = query_client.get_logs_config();
        let filter = solana_client::rpc_config::RpcTransactionLogsFilter::All;

        let (mut logs_subscription, _unsubscriber) = match client
            .logs_subscribe(filter, logs_config)
            .await
        {
            Ok(subscription) => subscription,
            Err(e) => {
                yield Err(TriggerError::SvmSubscription(e.into()));
                return;
            }
        };
        while let Some(log_response) = logs_subscription.next().await {
            let value = log_response.value;
            tracing::info!("Received SVM log event for signature: {}", value.signature);

            // Convert logs to our internal format
            let logs = value.logs;

            // Try to extract program ID from the first log line
            // Solana logs typically start with "Program <program_id> invoke [1]"
            let program_id = logs.iter()
                .find_map(|log| {
                    if log.starts_with("Program ") && log.contains(" invoke ") {
                        let parts: Vec<&str> = log.split_whitespace().collect();
                        if parts.len() >= 2 {
                            Some(parts[1].to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());

            let program_logs = vec![SvmProgramLog {
                program_id,
                logs,
                success: value.err.is_none(),
            }];

            // Parse events from the logs
            let all_logs: Vec<String> = program_logs.iter()
                .flat_map(|log| log.logs.iter().cloned())
                .collect();
            let parsed_events = parse_program_events(&all_logs);

            if !parsed_events.is_empty() || !all_logs.is_empty() {
                tracing::info!("Found {} parsed events and {} total logs", parsed_events.len(), all_logs.len());

                yield Ok(StreamTriggers::Svm {
                    chain_name: chain_name_clone.clone(),
                    signature: value.signature,
                    slot: log_response.context.slot,
                    program_logs: program_logs.clone(),
                    parsed_events,
                });
            }
        }

        tracing::warn!("SVM logs subscription ended");
    };

    Ok(Box::pin(event_stream))
}

/// Subscribes to program logs for a specific program ID
pub async fn subscribe_to_program_logs(
    _query_client: SvmQueryClient,
    _program_id: String,
    _chain_name: ChainName,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError> {
    // For now, return an empty stream as a placeholder
    // This will be implemented when SVM integration is ready
    let stream = stream::empty::<Result<StreamTriggers, TriggerError>>();
    Ok(Box::pin(stream))
}

/// Parse program logs to extract custom events
/// Looks for logs in format: "Program log: EVENT:<EventType>:<Data>"
pub fn parse_program_events(logs: &[String]) -> Vec<SvmParsedEvent> {
    let mut events = Vec::new();

    for log in logs {
        if let Some(event) = parse_single_event_log(log) {
            events.push(event);
        }
    }

    events
}

/// Parse a single log line to extract event data
/// Expected format: "Program log: EVENT:<EventType>:<Data>"
/// Where <Data> can be:
/// - Simple string: "Greeting"
/// - Key-value pairs: "from=Alice,to=Bob,amount=100"
pub fn parse_single_event_log(log: &str) -> Option<SvmParsedEvent> {
    // Check if this is an event log
    if !log.starts_with("Program log: EVENT:") {
        return None;
    }

    // Remove the prefix
    let event_str = log.trim_start_matches("Program log: EVENT:");

    // Split into event type and data
    let mut parts = event_str.splitn(2, ':');
    let event_type = parts.next()?.trim().to_string();
    let data_str = parts.next().unwrap_or("").trim();

    // Parse data as key-value pairs or simple string
    let mut data = BTreeMap::new();

    if data_str.is_empty() {
        // No additional data
    } else if data_str.contains('=') {
        // Parse as key-value pairs
        for pair in data_str.split(',') {
            if let Some((key, value)) = pair.split_once('=') {
                data.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    } else {
        // Treat as simple value
        data.insert("value".to_string(), data_str.to_string());
    }

    Some(SvmParsedEvent { event_type, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_event() {
        let log = "Program log: EVENT:Greeting:Simple greeting";
        let event = parse_single_event_log(log).unwrap();

        assert_eq!(event.event_type, "Greeting");
        assert_eq!(event.data.get("value"), Some(&"Simple greeting".to_string()));
    }

    #[test]
    fn test_parse_key_value_event() {
        let log = "Program log: EVENT:Transfer:from=Alice,to=Bob,amount=100";
        let event = parse_single_event_log(log).unwrap();

        assert_eq!(event.event_type, "Transfer");
        assert_eq!(event.data.get("from"), Some(&"Alice".to_string()));
        assert_eq!(event.data.get("to"), Some(&"Bob".to_string()));
        assert_eq!(event.data.get("amount"), Some(&"100".to_string()));
    }

    #[test]
    fn test_parse_empty_data_event() {
        let log = "Program log: EVENT:Ping:";
        let event = parse_single_event_log(log).unwrap();

        assert_eq!(event.event_type, "Ping");
        assert!(event.data.is_empty());
    }

    #[test]
    fn test_parse_non_event_log() {
        let log = "Program log: Hello, world!";
        let event = parse_single_event_log(log);

        assert!(event.is_none());
    }

    #[test]
    fn test_parse_multiple_events() {
        let logs = vec![
            "Program BXRTiRuo2tUh7ba9iYiwTNnrs3WJrEXuJZr2omDf44AF invoke [1]".to_string(),
            "Program log: EVENT:Greeting:Hello world".to_string(),
            "Program log: EVENT:Transfer:from=Alice,to=Bob,amount=100".to_string(),
            "Program BXRTiRuo2tUh7ba9iYiwTNnrs3WJrEXuJZr2omDf44AF consumed 153 of 200000 compute units".to_string(),
        ];

        let events = parse_program_events(&logs);
        assert_eq!(events.len(), 2);

        assert_eq!(events[0].event_type, "Greeting");
        assert_eq!(events[1].event_type, "Transfer");
    }
}
