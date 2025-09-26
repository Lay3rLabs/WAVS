use alloy_provider::Provider;
use alloy_rpc_types_eth::BlockNumberOrTag;
use alloy_rpc_types_eth::Filter as EthFilter;
use futures::Stream;
use std::pin::Pin;
use utils::{evm_client::EvmQueryClient, telemetry::TriggerMetrics};
use wavs_types::ChainKey;

use crate::subsystems::trigger::error::TriggerError;
use crate::subsystems::trigger::recovery::RecoveryManager;
use crate::subsystems::trigger::streams::StreamTriggers;

/// Start a ranged EVM event backfill stream using a provided filter from from_block to snapshot latest.
pub async fn start_event_backfill_stream(
    chain: ChainKey,
    client: EvmQueryClient,
    recovery_manager: std::sync::Arc<RecoveryManager>,
    filter: EthFilter,
    from_block: u64,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    // Snapshot latest at start
    let latest_snapshot = match client.provider.get_block_number().await {
        Ok(latest) => latest,
        Err(e) => {
            tracing::error!("Failed to get latest block for chain {}: {:?}", chain, e);
            return Err(TriggerError::EvmSubscription(e.into()));
        }
    };

    let range_start = from_block;
    let range_end = latest_snapshot;
    let chain_clone = chain.clone();
    let recovery = recovery_manager.clone();

    // Chunk size to avoid provider limits
    const CHUNK: u64 = 2_000;

    Ok(Box::pin(async_stream::try_stream! {
        let mut start = range_start;
        while start <= range_end {
            // Stop if recovery ended
            if let Some(state) = recovery.get_state(&chain_clone).await {
                if !state.is_in_recovery { break; }
            }

            let end = std::cmp::min(start + CHUNK - 1, range_end);
            let mut f = filter.clone();
            f = f.from_block(BlockNumberOrTag::Number(start.into())).to_block(BlockNumberOrTag::Number(end.into()));

            tracing::info!("Backfilling logs for chain {} blocks [{}..={}]", chain_clone, start, end);
            match client.provider.get_logs(&f).await {
                Ok(logs) => {
                    for log in logs {
                        if log.removed { continue; }
                        let block_timestamp = log.block_timestamp;
                        match (log.block_hash, log.transaction_index, log.block_number, log.transaction_hash, log.log_index) {
                            (Some(block_hash), Some(tx_index), Some(block_number), Some(tx_hash), Some(log_index)) => {
                                // Surface as Evm trigger
                                yield StreamTriggers::Evm {
                                    chain: chain_clone.clone(),
                                    block_number,
                                    tx_hash,
                                    log_index,
                                    log: Box::new(log),
                                    block_hash,
                                    block_timestamp,
                                    tx_index,
                                };

                                // Record block processed for recovery
                                recovery.record_successful_block(&chain_clone, block_number).await;
                            }
                            _ => {
                                tracing::debug!("Dropping incomplete EVM log during backfill");
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("get_logs backfill error on chain {} for range [{}..={}]: {:?}", chain_clone, start, end, e);
                    // small backoff to avoid hot loop
                    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                }
            }

            if end == u64::MAX { break; }
            start = end.saturating_add(1);
        }
    }))
}
