use anyhow::Result;
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_pubsub_client::nonblocking::pubsub_client::PubsubClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::str::FromStr;

use crate::error::SvmClientError;

#[derive(Clone)]
pub struct SvmQueryClient {
    pub endpoint: SvmEndpoint,
    pub commitment: Option<CommitmentConfig>,
}

#[derive(Debug, Clone)]
pub enum SvmEndpoint {
    WebSocket(String),
}

impl FromStr for SvmEndpoint {
    type Err = SvmClientError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = reqwest::Url::parse(s).map_err(|e| SvmClientError::ParseEndpoint(e.to_string()))?;
        match url.scheme() {
            "ws" | "wss" => Ok(SvmEndpoint::WebSocket(s.to_string())),
            scheme => Err(SvmClientError::ParseEndpoint(format!(
                "could not determine endpoint from scheme {scheme} (full url: {s}). SVM only supports WebSocket endpoints."
            ))),
        }
    }
}

impl SvmEndpoint {
    pub fn new_ws(url: &str) -> Result<Self, SvmClientError> {
        url.parse::<Self>().and_then(|endpoint| {
            if matches!(endpoint, SvmEndpoint::WebSocket(_)) {
                Ok(endpoint)
            } else {
                Err(SvmClientError::ParseEndpoint(
                    "url scheme is not ws or wss".to_string(),
                ))
            }
        })
    }

    pub async fn to_pubsub_client(&self) -> Result<PubsubClient, SvmClientError> {
        match self {
            SvmEndpoint::WebSocket(url) => {
                PubsubClient::new(url)
                    .await
                    .map_err(|e| SvmClientError::WebSocketClient(e.into()))
            }
        }
    }
}

impl SvmQueryClient {
    pub async fn new(endpoint: SvmEndpoint, commitment: Option<CommitmentConfig>) -> Result<Self, SvmClientError> {
        // Validate that we can connect to the endpoint
        let _client = endpoint.to_pubsub_client().await?;

        Ok(SvmQueryClient {
            endpoint,
            commitment,
        })
    }

    pub async fn create_program_logs_subscription(
        &self,
        _program_id: &str,
    ) -> Result<PubsubClient, SvmClientError> {
        let client = self.endpoint.to_pubsub_client().await?;
        Ok(client)
    }

    pub fn get_logs_filter(&self, program_id: &str) -> RpcTransactionLogsFilter {
        RpcTransactionLogsFilter::Mentions(vec![program_id.to_string()])
    }

    pub fn get_logs_config(&self) -> RpcTransactionLogsConfig {
        RpcTransactionLogsConfig {
            commitment: self.commitment,
        }
    }
}

impl std::fmt::Debug for SvmQueryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvmQueryClient")
            .field("endpoint", &self.endpoint)
            .field("commitment", &self.commitment)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_endpoint() {
        let endpoint = SvmEndpoint::from_str("ws://localhost:8900").unwrap();
        assert!(matches!(endpoint, SvmEndpoint::WebSocket(_)));

        let endpoint = SvmEndpoint::from_str("wss://api.mainnet-beta.solana.com").unwrap();
        assert!(matches!(endpoint, SvmEndpoint::WebSocket(_)));

        let endpoint = SvmEndpoint::from_str("http://localhost:8900").unwrap_err();
        assert!(matches!(endpoint, SvmClientError::ParseEndpoint(_)));
    }
}
