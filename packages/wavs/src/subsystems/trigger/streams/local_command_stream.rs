use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use utils::telemetry::TriggerMetrics;
use wavs_types::{ChainName, Trigger, TriggerConfig};

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

pub async fn start_local_command_stream(
    receiver: mpsc::UnboundedReceiver<LocalStreamCommand>,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let receiver = UnboundedReceiverStream::new(receiver);
    let command_stream = Box::pin(receiver.map(|command| {
        tracing::info!("Received local command: {:?}", command);
        Ok(StreamTriggers::LocalCommand(command))
    }));

    Ok(command_stream)
}

#[derive(Debug)]
pub enum LocalStreamCommand {
    StartListeningChain { chain_name: ChainName },
    StartListeningCron,
}

impl LocalStreamCommand {
    pub fn new(trigger_config: &TriggerConfig) -> Vec<Self> {
        trigger_config
            .triggers
            .iter()
            .filter_map(|trigger| match trigger {
                Trigger::Cron { .. } => Some(Self::StartListeningCron),
                Trigger::EvmContractEvent { chain_name, .. }
                | Trigger::CosmosContractEvent { chain_name, .. }
                | Trigger::BlockInterval { chain_name, .. } => Some(Self::StartListeningChain {
                    chain_name: chain_name.clone(),
                }),
                Trigger::Manual => None,
            })
            .collect()
    }
}
