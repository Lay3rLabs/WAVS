use std::sync::Arc;

use alloy::{
    network::EthereumWallet,
    primitives::Address,
    providers::{ProviderBuilder, RootProvider, WsConnect},
    signers::{
        k256::ecdsa::SigningKey,
        local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
    },
    transports::BoxTransport,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{eigen_client::solidity_types::BoxSigningProvider, error::EthClientError};

#[derive(Clone)]
pub struct EthQueryClient {
    pub config: EthClientConfig,
    pub provider: RootProvider<BoxTransport>,
}

impl std::fmt::Debug for EthQueryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthQueryClient")
            .field("ws_endpoint", &self.config.ws_endpoint)
            .field("http_endpoint", &self.config.http_endpoint)
            .finish()
    }
}

#[derive(Clone)]
pub struct EthSigningClient {
    pub config: EthClientConfig,
    pub provider: BoxSigningProvider,
    /// The wallet is a collection of signers, with one designated as the default signer
    /// it allows signing transactions
    pub wallet: Arc<EthereumWallet>,
    /// The signer is the same as the default signer in the wallet, but used for simple message signing
    /// due to type system limitations, we need to store it separately
    /// since the signer in `EthereumWallet` implements only `TxSigner`
    /// and there is not a direct way convert it into `Signer`
    pub signer: Arc<LocalSigner<SigningKey>>,
}

impl std::fmt::Debug for EthSigningClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthSigningClient")
            .field("ws_endpoint", &self.config.ws_endpoint)
            .field("http_endpoint", &self.config.http_endpoint)
            .field("address", &self.address())
            .finish()
    }
}

impl EthSigningClient {
    pub fn address(&self) -> Address {
        self.signer.address()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EthClientConfig {
    pub ws_endpoint: Option<String>,
    pub http_endpoint: String,
    pub mnemonic: Option<String>,
    pub hd_index: Option<u32>,
    /// Preferred transport
    pub transport: Option<EthClientTransport>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum EthClientTransport {
    WebSocket,
    Http,
}

pub struct EthClientBuilder {
    pub config: EthClientConfig,
}

impl EthClientBuilder {
    pub fn new(config: EthClientConfig) -> Self {
        Self { config }
    }

    fn preferred_transport(&self) -> EthClientTransport {
        match (self.config.transport, &self.config.ws_endpoint) {
            // Http preferred or no preference and no websocket
            (Some(EthClientTransport::Http), _) | (None, None) => EthClientTransport::Http,
            // Otherwise try to connect to websocket
            _ => EthClientTransport::WebSocket,
        }
    }

    pub async fn build_query(self) -> Result<EthQueryClient> {
        let provider: RootProvider<BoxTransport> = match self.preferred_transport() {
            // Http preferred or no preference and no websocket
            EthClientTransport::Http => {
                let endpoint_url = self.config.http_endpoint.parse()?;
                ProviderBuilder::new().on_http(endpoint_url).boxed()
            }
            EthClientTransport::WebSocket => {
                let ws =
                    WsConnect::new(self.config.ws_endpoint.as_ref().context(
                        "Websocket is preferred transport, but endpoint was not provided",
                    )?);
                ProviderBuilder::new().on_ws(ws).await?.boxed()
            }
        };

        Ok(EthQueryClient {
            config: self.config,
            provider,
        })
    }

    pub async fn build_signing(mut self) -> Result<EthSigningClient> {
        let mnemonic = self
            .config
            .mnemonic
            .take()
            .ok_or(EthClientError::MissingMnemonic)?;

        let signer = MnemonicBuilder::<English>::default()
            .phrase(mnemonic)
            .index(self.config.hd_index.unwrap_or(0))?
            .build()?;

        let wallet: EthereumWallet = signer.clone().into();
        let query_client = self.build_query().await?;
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet.clone())
            .on_provider(query_client.provider);

        Ok(EthSigningClient {
            config: query_client.config,
            provider,
            wallet: Arc::new(wallet),
            signer: Arc::new(signer),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn preferred_transport() {
        // Not specified preference, websocket provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: "bar".to_owned(),
            transport: None,
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::WebSocket));

        // Not specified preference, websocket not provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: None,
            http_endpoint: "bar".to_owned(),
            transport: None,
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::Http));

        // Specified Http preference, websocket provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: "bar".to_owned(),
            transport: Some(EthClientTransport::Http),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::Http));

        // Specified Http preference, websocket not provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: None,
            http_endpoint: "bar".to_owned(),
            transport: Some(EthClientTransport::Http),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::Http));

        // Specified Websocket preference, websocket provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: "bar".to_owned(),
            transport: Some(EthClientTransport::WebSocket),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::WebSocket));

        // Specified Websocket preference, websocket not provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: None,
            http_endpoint: "bar".to_owned(),
            transport: Some(EthClientTransport::WebSocket),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::WebSocket));
    }
}
