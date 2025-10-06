use alloy_rpc_types_eth::Log;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio_stream::wrappers::UnboundedReceiverStream;
use utils::telemetry::TriggerMetrics;
use wavs_types::ChainKey;

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

pub async fn start_evm_event_stream(
    chain: ChainKey,
    log_stream: UnboundedReceiverStream<Log>,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let chain = chain.clone();

    let event_stream = Box::pin(log_stream.filter_map(move |log| {
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
    chain: ChainKey,
    block_height_stream: UnboundedReceiverStream<u64>,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let block_stream = Box::pin(block_height_stream.map(move |block_height| {
        Ok(StreamTriggers::EvmBlock {
            chain: chain.clone(),
            block_height,
        })
    }));

    Ok(block_stream)
}
