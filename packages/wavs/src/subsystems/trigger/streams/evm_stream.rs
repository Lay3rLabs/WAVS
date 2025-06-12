use alloy_provider::Provider;
use alloy_rpc_types_eth::Filter;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use utils::{error::EvmClientError, evm_client::EvmQueryClient, telemetry::TriggerMetrics};
use wavs_types::ChainName;

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

pub async fn start_evm_event_stream(
    query_client: EvmQueryClient,
    chain_name: ChainName,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let filter = Filter::new();

    let stream = query_client
        .provider
        .subscribe_logs(&filter)
        .await
        .map_err(|e| TriggerError::EvmSubscription(e.into()))?
        .into_stream();

    let chain_name = chain_name.clone();

    let event_stream = Box::pin(stream.map(move |log| {
        Ok(StreamTriggers::Evm {
            chain_name: chain_name.clone(),
            block_height: log.block_number.ok_or_else(|| {
                TriggerError::EvmClient(chain_name.clone(), EvmClientError::BlockHeight)
            })?,
            log,
        })
    }));

    Ok(event_stream)
}

pub async fn start_evm_block_stream(
    query_client: EvmQueryClient,
    chain_name: ChainName,
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
            chain_name: chain_name.clone(),
            block_height: block.number,
        })
    }));

    Ok(block_stream)
}
