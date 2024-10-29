use std::sync::Arc;

use crate::{apis::dispatcher::DispatchManager, context::AppContext, dispatcher::DispatcherError};

#[derive(Clone)]
pub struct HttpState {
    pub ctx: AppContext,
    pub dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
}

impl HttpState {
    pub async fn new(
        ctx: AppContext,
        dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
    ) -> anyhow::Result<Self> {
        Ok(Self { ctx, dispatcher })
    }
}
