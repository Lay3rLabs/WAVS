use std::sync::Arc;

use crate::config::Config;

#[derive(Clone)]
pub struct HttpState {
    pub config: Arc<Config>,
}

impl HttpState {
    pub async fn new(config: Arc<Config>) -> anyhow::Result<Self> {
        Ok(Self { config })
    }
}
