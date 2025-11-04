use futures::{Stream, StreamExt};
use layer_climb::prelude::*;
use std::pin::Pin;
use utils::telemetry::TriggerMetrics;
use wavs_types::ChainKey;

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

pub async fn start_cosmos_stream(
    query_client: QueryClient,
    chain: ChainKey,
    metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let stream = Box::pin(
        query_client
            .clone()
            .stream_block_events(None)
            .await
            .map_err(TriggerError::Climb)?
            .map(move |block_events| match block_events {
                Ok(block_events) => {
                    let mut contract_events = Vec::new();
                    let events = CosmosTxEvents::from(block_events.events);

                    for (index, event) in events.events_iter().enumerate() {
                        if event.ty().starts_with("wasm-") {
                            let contract_address = event.attributes().find_map(|attr| {
                                if attr.key() == "_contract_address"
                                    || attr.key() == "contract_address"
                                    || attr.key() == "_contract_addr"
                                    || attr.key() == "contract_addr"
                                {
                                    query_client
                                        .chain_config
                                        .parse_address(attr.value())
                                        .ok()
                                        .and_then(|addr| CosmosAddr::try_from(addr).ok())
                                } else {
                                    None
                                }
                            });
                            match contract_address {
                                Some(contract_address) => {
                                    let mut event = cosmwasm_std::Event::from(event);
                                    event.ty = event.ty.strip_prefix("wasm-").unwrap().to_string();
                                    contract_events.push(StreamTriggerCosmosContractEvent {
                                        contract_address,
                                        event,
                                        event_index: index
                                            .try_into()
                                            .map_err(TriggerError::EventIndexConversion)?,
                                    });
                                }
                                None => {
                                    tracing::warn!(
                                        "Missing contract address in event: {:?}",
                                        event
                                    );
                                }
                            }
                        }
                    }

                    Ok(StreamTriggers::Cosmos {
                        chain: chain.clone(),
                        contract_events,
                        block_height: block_events.height,
                    })
                }
                Err(err) => {
                    metrics.increment_total_errors("block_events");
                    Err(TriggerError::Climb(err))
                }
            }),
    );

    Ok(stream)
}

#[derive(Debug)]
pub struct StreamTriggerCosmosContractEvent {
    pub contract_address: layer_climb::prelude::CosmosAddr,
    pub event: cosmwasm_std::Event,
    pub event_index: u64,
}
