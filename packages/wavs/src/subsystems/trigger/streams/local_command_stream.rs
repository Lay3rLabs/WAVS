use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use utils::telemetry::TriggerMetrics;
use wavs_types::{ChainKey, Trigger, TriggerAction, TriggerConfig};

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
    StartListeningChain { chain: ChainKey },
    StartListeningCron,
    ManualTrigger(Box<TriggerAction>),
}

impl LocalStreamCommand {
    pub fn new(trigger_config: &TriggerConfig) -> Option<Self> {
        match &trigger_config.trigger {
            Trigger::Cron { .. } => Some(Self::StartListeningCron),
            Trigger::EvmContractEvent { chain, .. }
            | Trigger::CosmosContractEvent { chain, .. }
            | Trigger::BlockInterval { chain, .. } => Some(Self::StartListeningChain {
                chain: chain.clone(),
            }),
            Trigger::Manual => None,
        }
    }
}
