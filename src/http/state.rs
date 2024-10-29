use std::sync::Arc;

use crate::{apis::dispatcher::DispatchManager, config::Config, dispatcher::DispatcherError};

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
}

impl HttpState {
    pub async fn new(
        config: Config,
        dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
    ) -> anyhow::Result<Self> {
        Ok(Self { config, dispatcher })
    }
}
