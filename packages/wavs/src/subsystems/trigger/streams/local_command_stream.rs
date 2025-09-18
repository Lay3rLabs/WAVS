use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use utils::telemetry::TriggerMetrics;

use crate::subsystems::trigger::{error::TriggerError, TriggerCommand};

use super::StreamTriggers;

pub fn start_local_command_stream(
    receiver: mpsc::UnboundedReceiver<TriggerCommand>,
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
