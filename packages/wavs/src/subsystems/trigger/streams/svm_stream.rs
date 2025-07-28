use futures::{stream, Stream, StreamExt};
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
    _query_client: SvmQueryClient,
    chain_name: ChainName,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    // Create a placeholder stream that will be replaced with actual subscription logic
    // This structure follows the pattern of EVM and Cosmos streams
    let chain_name_clone = chain_name.clone();

    // For now, create an empty stream that will be replaced with actual Solana subscription
    let stream = stream::empty::<()>();

    let event_stream: Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>> = Box::pin(stream.filter_map(move |_: ()| {
        let _chain_name = chain_name_clone.clone();
        async move {
            // This will be replaced with actual log processing
            None::<Result<StreamTriggers, TriggerError>>
        }
    }));

    Ok(event_stream)
}

/// Subscribes to program logs for a specific program ID
pub async fn subscribe_to_program_logs(
    query_client: SvmQueryClient,
    program_id: String,
    chain_name: ChainName,
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
