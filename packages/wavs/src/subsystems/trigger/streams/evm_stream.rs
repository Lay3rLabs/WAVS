use alloy_provider::Provider;
use alloy_rpc_types_eth::Filter;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use utils::{evm_client::EvmQueryClient, telemetry::TriggerMetrics};
use wavs_types::ChainKey;

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

/// Start a resilient EVM event stream that auto-reconnects and can accept a narrowed filter.
pub async fn start_evm_stream(
    query_client: EvmQueryClient,
    chain: ChainKey,
    filter: Filter,
    _metrics: TriggerMetrics,
    cancel: CancellationToken,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let provider = query_client.provider.clone();
    let chain_cloned = chain.clone();

    let stream = async_stream::stream! {
        let mut backoff_ms = 500u64;
        loop {
            tracing::info!(target: "wavs::evm_stream", chain = %chain_cloned, "Subscribing to logs with filter: {:?}", filter);
            match provider.subscribe_logs(&filter).await {
                Ok(sub) => {
                    backoff_ms = 500; // reset backoff on success
                    let mut inner = sub.into_stream();
                    let mut should_exit = false;
                    loop {
                        tokio::select! {
                            biased;
                            // shutdown first
                            _ = cancel.cancelled() => {
                                tracing::info!("EVM log subscription received shutdown signal");
                                should_exit = true;
                                break;
                            }
                            maybe_log = inner.next() => {
                                match maybe_log {
                                    Some(log) => {
                                        if log.removed { continue; }
                                        let block_timestamp = log.block_timestamp;
                                        match (log.block_hash, log.transaction_index, log.block_number, log.transaction_hash, log.log_index) {
                                            (Some(block_hash), Some(tx_index), Some(block_number), Some(tx_hash), Some(log_index)) => {
                                                yield Ok(StreamTriggers::Evm {
                                                    chain: chain_cloned.clone(),
                                                    block_number,
                                                    tx_hash,
                                                    log_index,
                                                    log: Box::new(log),
                                                    block_hash,
                                                    block_timestamp,
                                                    tx_index,
                                                });
                                            }
                                            _ => {
                                                tracing::debug!("Dropping incomplete EVM log: {:?}", log);
                                            }
                                        }
                                    }
                                    None => break,
                                }
                            }
                        }
                    }
                    if should_exit {
                        tracing::info!("EVM log subscription exiting after shutdown");
                        break;
                    } else {
                        // EOF or subscription ended. Surface a soft error and retry.
                        tracing::warn!("EVM log subscription ended; reconnecting...");
                        yield Err(TriggerError::EvmSubscription(anyhow::anyhow!("log subscription ended")));
                    }
                }
                Err(e) => {
                    tracing::error!("EVM subscribe_logs error: {:?}", e);
                    yield Err(TriggerError::EvmSubscription(e.into()));
                }
            }

            // Exponential backoff before resubscribing
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms.saturating_mul(2)).min(10_000);
        }
    };

    Ok(Box::pin(stream))
}

pub async fn start_evm_block_stream(
    query_client: EvmQueryClient,
    chain: ChainKey,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let provider = query_client.provider.clone();
    let chain_cloned = chain.clone();

    let stream = async_stream::stream! {
        let mut backoff_ms = 500u64;
        loop {
            match provider.subscribe_blocks().await {
                Ok(sub) => {
                    backoff_ms = 500;
                    let mut inner = sub.into_stream();
                    while let Some(block) = inner.next().await {
                        yield Ok(StreamTriggers::EvmBlock { chain: chain_cloned.clone(), block_height: block.number });
                    }
                    tracing::warn!("EVM block subscription ended; reconnecting...");
                    yield Err(TriggerError::EvmSubscription(anyhow::anyhow!("block subscription ended")));
                }
                Err(e) => {
                    tracing::error!("EVM subscribe_blocks error: {:?}", e);
                    yield Err(TriggerError::EvmSubscription(e.into()));
                }
            }

            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms.saturating_mul(2)).min(10_000);
        }
    };

    Ok(Box::pin(stream))
}
