use alloy_provider::Provider;
use alloy_rpc_types_eth::Filter;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use utils::{evm_client::EvmQueryClient, telemetry::TriggerMetrics};
use wavs_types::ChainKey;

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

const DEFAULT_ALLOY_CHANNEL_SIZE: usize = 16;

pub async fn start_evm_event_stream(
    query_client: EvmQueryClient,
    chain: ChainKey,
    channel_size: usize,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let filter = Filter::new();
    // Minimum default Alloy configuration
    let channel_size = channel_size.max(DEFAULT_ALLOY_CHANNEL_SIZE);
    let stream = query_client
        .provider
        .subscribe_logs(&filter)
        .channel_size(channel_size)
        .await
        .map_err(|e| TriggerError::EvmSubscription(e.into()))?
        .into_stream();

    let chain = chain.clone();

    let event_stream = Box::pin(stream.filter_map(move |log| {
        let chain = chain.clone();
        async move {
            if log.removed {
                tracing::warn!("Reorg removed log: {:?}", log);
                return None;
            }

            let block_timestamp = log.block_timestamp;

            match (
                log.block_hash,
                log.transaction_index,
                log.block_number,
                log.transaction_hash,
                log.log_index,
            ) {
                (
                    Some(block_hash),
                    Some(tx_index),
                    Some(block_number),
                    Some(tx_hash),
                    Some(log_index),
                ) => Some(Ok(StreamTriggers::Evm {
                    chain: chain.clone(),
                    block_number,
                    tx_hash,
                    log_index,
                    log: Box::new(log),
                    block_hash,
                    block_timestamp,
                    tx_index,
                })),
                _ => {
                    tracing::warn!("Received incomplete EVM log: {:?}", log);
                    None
                }
            }
        }
    }));

    Ok(event_stream)
}

pub async fn start_evm_block_stream(
    query_client: EvmQueryClient,
    chain: ChainKey,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    // Start the block stream (for block-based triggers)
    let stream = query_client
        .provider
        .subscribe_blocks()
        .await
        .map_err(|e| TriggerError::EvmSubscription(e.into()))?
        .into_stream();

    let block_stream = Box::pin(stream.map(move |block| {
        Ok(StreamTriggers::EvmBlock {
            chain: chain.clone(),
            block_height: block.number,
        })
    }));

    Ok(block_stream)
}
