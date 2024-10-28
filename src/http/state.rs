use crate::dispatcher::Dispatcher;

#[derive(Clone)]
pub struct HttpState {
    pub dispatcher: Dispatcher,
}

impl HttpState {
    pub async fn new(dispatcher: Dispatcher) -> anyhow::Result<Self> {
        Ok(Self { dispatcher })
    }
}
