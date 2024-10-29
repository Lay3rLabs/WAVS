use std::sync::Arc;

use crate::{apis::dispatcher::DispatchManager, dispatcher::core::DispatcherError};

#[derive(Clone)]
pub struct HttpState {
    pub dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
}

impl HttpState {
    pub async fn new(
        dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
    ) -> anyhow::Result<Self> {
        Ok(Self { dispatcher })
    }
}
