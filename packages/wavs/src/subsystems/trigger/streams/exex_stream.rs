//! ExEx (Execution Extension) stream for receiving blockchain execution data from reth via gRPC.
//!
//! This module provides a gRPC client that connects to a reth node's remote ExEx server
//! and receives execution notifications (committed chains, reorgs, reverts) directly,
//! bypassing the standard WebSocket JSON-RPC interface.

use alloy_consensus::{BlockHeader as AlloyBlockHeader, TxReceipt};
use alloy_primitives::Log;
use futures::{Stream, StreamExt};
use reth_exex_types::ExExNotification;
use reth_primitives_traits::BlockBody as BlockBodyTrait;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::pin::Pin;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

use crate::subsystems::trigger::error::TriggerError;
use crate::subsystems::trigger::streams::StreamTriggers;
use utils::telemetry::TriggerMetrics;
use wavs_types::ChainKey;

/// Generated gRPC client code from proto/exex.proto
pub mod proto {
    tonic::include_proto!("exex");
}

/// Configuration for ExEx gRPC connection
#[derive(Debug, Clone)]
pub struct ExExConfig {
    /// gRPC endpoint URL (e.g., "http://[::1]:10000")
    pub endpoint: String,
    /// Chain key identifying this EVM chain
    pub chain: ChainKey,
}

/// Wrapper struct for deserializing ExExNotification using serde_bincode_compat
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
struct ExExNotificationWrapper {
    #[serde_as(
        as = "reth_exex_types::serde_bincode_compat::ExExNotification<'_, reth_ethereum_primitives::EthPrimitives>"
    )]
    notification: ExExNotification<reth_ethereum_primitives::EthPrimitives>,
}

/// Create an ExEx stream for receiving execution notifications from reth
///
/// This function establishes a gRPC connection to the reth ExEx server and
/// returns a stream of `StreamTriggers` events derived from execution notifications.
///
/// # Arguments
/// * `config` - ExEx configuration including endpoint and chain key
/// * `metrics` - Metrics collector for monitoring
///
/// # Returns
/// A pinned stream that yields `StreamTriggers` for EVM events and blocks
pub async fn start_exex_stream(
    config: ExExConfig,
    metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let stream = async_stream::stream! {
        let mut reconnect_count = 0;
        let max_reconnects = 10;
        let base_delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(60);

        loop {
            info!("Connecting to ExEx at: {}", config.endpoint);

            match create_exex_connection(&config).await {
                Ok(mut stream) => {
                    reconnect_count = 0;
                    info!("Successfully connected to ExEx server");
                    metrics.increment_total_errors("exex_connection_success");

                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(notification_proto) => {
                                metrics.increment_total_errors("exex_message_received");

                                // Deserialize the bincode-encoded ExExNotification
                                match deserialize_notification(&notification_proto.data) {
                                    Ok(notification) => {
                                        // Convert notification to stream triggers
                                        let triggers = convert_notification(notification, &config.chain);
                                        for trigger in triggers {
                                            metrics.increment_total_errors("exex_trigger_emitted");
                                            yield Ok(trigger);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to deserialize ExEx notification: {:?}", e);
                                        metrics.increment_total_errors("exex_deserialization_error");
                                        // Continue processing - don't break connection for deserialization errors
                                    }
                                }
                            }
                            Err(e) => {
                                error!("ExEx stream error: {:?}", e);
                                metrics.increment_total_errors("exex_stream_error");
                                break; // Break from inner loop to trigger reconnect
                            }
                        }
                    }
                    info!("ExEx stream ended, will attempt reconnection");
                }
                Err(e) => {
                    error!("ExEx connection error: {:?}", e);
                    metrics.increment_total_errors("exex_connection_error");

                    if reconnect_count >= max_reconnects {
                        error!("Max reconnection attempts reached, giving up");
                        metrics.increment_total_errors("exex_max_reconnects_reached");
                        yield Err(TriggerError::ExExConnection("Max reconnection attempts reached".to_string()));
                        return;
                    }

                    // Exponential backoff with jitter
                    let delay = std::cmp::min(
                        base_delay * 2_u32.pow(reconnect_count),
                        max_delay
                    ) + Duration::from_millis(rand::random::<u64>() % 1000);

                    warn!("Reconnecting in {:?} (attempt {})", delay, reconnect_count + 1);
                    metrics.increment_total_errors("exex_reconnect_attempt");
                    sleep(delay).await;
                    reconnect_count += 1;
                }
            }
        }
    };

    Ok(Box::pin(stream))
}

/// Create a new gRPC connection to the ExEx server
async fn create_exex_connection(
    config: &ExExConfig,
) -> Result<tonic::Streaming<proto::ExExNotification>, TriggerError> {
    let mut client = proto::remote_ex_ex_client::RemoteExExClient::connect(config.endpoint.clone())
        .await
        .map_err(|e| TriggerError::ExExConnection(format!("Failed to connect: {}", e)))?
        .max_encoding_message_size(usize::MAX)
        .max_decoding_message_size(usize::MAX);

    let stream = client
        .subscribe(proto::SubscribeRequest {})
        .await
        .map_err(|e| TriggerError::ExExConnection(format!("Failed to subscribe: {}", e)))?
        .into_inner();

    Ok(stream)
}

/// Deserialize bincode-encoded ExExNotification using serde_bincode_compat
fn deserialize_notification(
    data: &[u8],
) -> Result<ExExNotification<reth_ethereum_primitives::EthPrimitives>, TriggerError> {
    // Use bincode 2.x API with standard configuration and serde_bincode_compat wrapper
    let config = bincode::config::standard();
    let wrapper: ExExNotificationWrapper = bincode::serde::decode_from_slice(data, config)
        .map(|(wrapper, _)| wrapper)
        .map_err(|e| TriggerError::ExExDeserialization(format!("Bincode decode error: {}", e)))?;
    Ok(wrapper.notification)
}

/// Convert an ExExNotification into StreamTriggers
///
/// Processing rules:
/// - ChainCommitted: Process the new chain, emit triggers for all logs
/// - ChainReorged: Process only the new chain (old chain is reverted)
/// - ChainReverted: Skip entirely (no new events to emit)
fn convert_notification(
    notification: ExExNotification<reth_ethereum_primitives::EthPrimitives>,
    chain: &ChainKey,
) -> Vec<StreamTriggers> {
    let mut triggers = Vec::new();

    // Get the committed chain (if any)
    let committed_chain = match &notification {
        ExExNotification::ChainCommitted { new } => Some(new.clone()),
        ExExNotification::ChainReorged { new, .. } => Some(new.clone()),
        ExExNotification::ChainReverted { .. } => None,
    };

    let Some(chain_data) = committed_chain else {
        return triggers;
    };

    // Process each block and its receipts
    for (block, receipts) in chain_data.blocks_and_receipts() {
        let header = block.header();
        let block_number = AlloyBlockHeader::number(header);
        let block_hash = block.hash();
        let block_timestamp = AlloyBlockHeader::timestamp(header);

        // Emit block trigger
        triggers.push(StreamTriggers::EvmBlock {
            chain: chain.clone(),
            block_height: block_number,
        });

        // Process each transaction's receipts for logs
        let transactions = BlockBodyTrait::transactions(block.body());
        for (tx_index, (tx, receipt)) in transactions
            .iter()
            .zip(receipts.iter())
            .enumerate()
        {
            let tx_hash: alloy_primitives::TxHash = *tx.tx_hash();

            // Extract logs from receipt
            let logs = TxReceipt::logs(receipt);
            for (log_index_in_tx, log) in logs.iter().enumerate() {
                // Calculate global log index within the block
                let log_index = calculate_log_index(receipts, tx_index, log_index_in_tx);

                triggers.push(StreamTriggers::Evm {
                    chain: chain.clone(),
                    log: Box::new(convert_log(log, block_number, block_hash, tx_hash, tx_index as u64, log_index)),
                    block_number,
                    tx_hash,
                    block_hash,
                    tx_index: tx_index as u64,
                    block_timestamp: Some(block_timestamp),
                    log_index,
                });
            }
        }
    }

    triggers
}

/// Calculate the global log index within a block
fn calculate_log_index(
    receipts: &[reth_ethereum_primitives::Receipt],
    tx_index: usize,
    log_index_in_tx: usize,
) -> u64 {
    let mut total_logs: u64 = 0;
    for (i, receipt) in receipts.iter().enumerate() {
        if i == tx_index {
            return total_logs + log_index_in_tx as u64;
        }
        total_logs += TxReceipt::logs(receipt).len() as u64;
    }
    total_logs + log_index_in_tx as u64
}

/// Convert a reth Log to an alloy_rpc_types_eth::Log
fn convert_log(
    log: &Log,
    block_number: u64,
    block_hash: alloy_primitives::BlockHash,
    tx_hash: alloy_primitives::TxHash,
    tx_index: u64,
    log_index: u64,
) -> alloy_rpc_types_eth::Log {
    alloy_rpc_types_eth::Log {
        inner: log.clone(),
        block_hash: Some(block_hash),
        block_number: Some(block_number),
        block_timestamp: None,
        transaction_hash: Some(tx_hash),
        transaction_index: Some(tx_index),
        log_index: Some(log_index),
        removed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exex_config_creation() {
        let config = ExExConfig {
            endpoint: "http://[::1]:10000".to_string(),
            chain: "evm:local".parse().unwrap(),
        };
        assert_eq!(config.endpoint, "http://[::1]:10000");
    }
}
