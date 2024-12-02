use crate::config::Config;

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
}

impl HttpState {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        Ok(Self { config })
    }
}
