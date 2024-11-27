use crate::config::Config;

#[derive(Clone)]
pub struct AggregatorState {}

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub aggregator: AggregatorState,
    pub is_mock_chain_client: bool,
}

impl HttpState {
    pub async fn new(
        config: Config,
        aggregator: AggregatorState,
        is_mock_chain_client: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            config,
            aggregator,
            is_mock_chain_client,
        })
    }
}
