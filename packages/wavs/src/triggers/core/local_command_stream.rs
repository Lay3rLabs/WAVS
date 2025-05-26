use crate::apis::trigger::TriggerError;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use utils::telemetry::TriggerMetrics;

use super::{LocalStreamCommand, StreamTriggers};

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
