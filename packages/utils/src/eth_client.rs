use std::sync::Arc;

use alloy::{
    network::{Ethereum, EthereumWallet},
    primitives::Address,
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, ProviderBuilder, RootProvider,
    },
    signers::{
        k256::ecdsa::SigningKey,
        local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
    },
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::error::EthClientError;

#[derive(Clone)]
pub struct EthQueryClient {
    pub config: EthClientConfig,
    pub provider: QueryProvider,
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
    pub provider: SigningProvider,
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
    pub http_endpoint: Option<String>,
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

    pub fn endpoint(&self) -> Result<String> {
        match self.preferred_transport() {
            // Http preferred or no preference and no websocket
            EthClientTransport::Http => Ok(self
                .config
                .http_endpoint
                .as_ref()
                .context("no http endpoint")?
                .to_string()),
            EthClientTransport::WebSocket => Ok(self
                .config
                .ws_endpoint
                .as_ref()
                .context("Websocket is preferred transport, but endpoint was not provided")?
                .to_string()),
        }
    }
    pub async fn build_query(self) -> Result<EthQueryClient> {
        let endpoint = self.endpoint()?;

        Ok(EthQueryClient {
            config: self.config,
            provider: ProviderBuilder::new().on_builtin(&endpoint).await?,
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

        let endpoint = self.endpoint()?;

        let provider = ProviderBuilder::new()
            .wallet(wallet.clone())
            .on_builtin(&endpoint)
            .await?;

        Ok(EthSigningClient {
            config: self.config,
            provider,
            wallet: Arc::new(wallet),
            signer: Arc::new(signer),
        })
    }
}

pub type QueryProvider = FillProvider<
    JoinFill<
        Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider,
    Ethereum,
>;
pub type SigningProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
    Ethereum,
>;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn preferred_transport() {
        // Not specified preference, websocket provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: Some("bar".to_owned()),
            transport: None,
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::WebSocket));

        // Not specified preference, websocket not provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: None,
            http_endpoint: Some("bar".to_owned()),
            transport: None,
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::Http));

        // Specified Http preference, websocket provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EthClientTransport::Http),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::Http));

        // Specified Http preference, websocket not provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: None,
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EthClientTransport::Http),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::Http));

        // Specified Websocket preference, websocket provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EthClientTransport::WebSocket),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::WebSocket));

        // Specified Websocket preference, websocket not provided
        let transport = EthClientBuilder::new(EthClientConfig {
            ws_endpoint: None,
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EthClientTransport::WebSocket),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EthClientTransport::WebSocket));
    }
}
