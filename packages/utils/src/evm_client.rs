pub mod contracts;
pub mod signing;

use alloy_network::{EthereumWallet, Network};
use alloy_primitives::Address;
use alloy_provider::{
    fillers::{BlobGasFiller, ChainIdFiller, GasFiller, NonceManager},
    DynProvider, Provider, ProviderBuilder,
};
use alloy_signer_local::PrivateKeySigner;
use alloy_transport::{TransportErrorKind, TransportResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use signing::make_signer;
use std::sync::{atomic::AtomicU64, Arc};

use crate::error::EvmClientError;

#[derive(Clone)]
pub struct EvmQueryClient {
    pub config: EvmClientConfig,
    pub provider: DynProvider,
}

impl std::fmt::Debug for EvmQueryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmQueryClient")
            .field("ws_endpoint", &self.config.ws_endpoint)
            .field("http_endpoint", &self.config.http_endpoint)
            .finish()
    }
}

#[derive(Clone)]
pub struct EvmSigningClient {
    pub config: EvmClientConfig,
    pub provider: DynProvider,
    /// The wallet is a collection of signers, with one designated as the default signer
    /// it allows signing transactions
    pub wallet: Arc<EthereumWallet>,
    /// The signer is the same as the default signer in the wallet, but used for simple message signing
    /// due to type system limitations, we need to store it separately
    /// since the signer in `EthereumWallet` implements only `TxSigner`
    /// and there is not a direct way convert it into `Signer`
    pub signer: Arc<PrivateKeySigner>,

    /// If a transaction does not have `max_gas` set, then it will estimate
    /// however the actual gas needed fluctuates, so we can pad it with a multiplier
    /// by default this is 1.25
    pub gas_estimate_multiplier: f32,
}

impl std::fmt::Debug for EvmSigningClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmSigningClient")
            .field("ws_endpoint", &self.config.ws_endpoint)
            .field("http_endpoint", &self.config.http_endpoint)
            .field("address", &self.address())
            .finish()
    }
}

impl EvmSigningClient {
    pub fn address(&self) -> Address {
        self.signer.address()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EvmClientConfig {
    pub ws_endpoint: Option<String>,
    pub http_endpoint: Option<String>,
    pub credential: Option<String>,
    pub hd_index: Option<u32>,
    /// Preferred transport
    pub transport: Option<EvmClientTransport>,
    // if not set, will be 1.25
    pub gas_estimate_multiplier: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub enum EvmClientTransport {
    WebSocket,
    Http,
}

pub struct EvmClientBuilder {
    pub config: EvmClientConfig,
}

impl EvmClientBuilder {
    pub fn new(config: EvmClientConfig) -> Self {
        Self { config }
    }

    fn preferred_transport(&self) -> EvmClientTransport {
        match (self.config.transport, &self.config.ws_endpoint) {
            // Http preferred or no preference and no websocket
            (Some(EvmClientTransport::Http), _) | (None, None) => EvmClientTransport::Http,
            // Otherwise try to connect to websocket
            _ => EvmClientTransport::WebSocket,
        }
    }

    pub fn endpoint(&self) -> Result<String> {
        match self.preferred_transport() {
            // Http preferred or no preference and no websocket
            EvmClientTransport::Http => Ok(self
                .config
                .http_endpoint
                .as_ref()
                .context("no http endpoint")?
                .to_string()),
            EvmClientTransport::WebSocket => Ok(self
                .config
                .ws_endpoint
                .as_ref()
                .context("Websocket is preferred transport, but endpoint was not provided")?
                .to_string()),
        }
    }
    pub async fn build_query(self) -> Result<EvmQueryClient> {
        let endpoint = self.endpoint()?;

        Ok(EvmQueryClient {
            config: self.config,
            provider: DynProvider::new(ProviderBuilder::new().connect(&endpoint).await?),
        })
    }

    pub async fn build_signing(mut self) -> Result<EvmSigningClient> {
        if self.preferred_transport() != EvmClientTransport::Http {
            tracing::warn!("signing clients should probably prefer http transport");
        }

        let credentials = self
            .config
            .credential
            .take()
            .ok_or(EvmClientError::MissingMnemonic)?;

        let signer = make_signer(&credentials, self.config.hd_index)?;

        let wallet: EthereumWallet = signer.clone().into();

        let endpoint = self.endpoint()?;

        let query_provider = ProviderBuilder::new().connect(&endpoint).await?;
        let first_nonce = query_provider
            .get_transaction_count(signer.address())
            .await?;

        let nonce_manager = FastNonceManager::new(Some(signer.address()), first_nonce);

        let provider = DynProvider::new(
            ProviderBuilder::default()
                .with_nonce_management(nonce_manager)
                .filler(GasFiller)
                .filler(BlobGasFiller)
                .filler(ChainIdFiller::new(None))
                .wallet(wallet.clone())
                .connect(&endpoint)
                .await?,
        );

        // default
        // let provider = DynProvder::new(ProviderBuilder::new()
        //         .wallet(wallet.clone())
        //         .on_builtin(&endpoint)
        //         .await?);

        Ok(EvmSigningClient {
            gas_estimate_multiplier: self.config.gas_estimate_multiplier.unwrap_or(1.25),
            config: self.config,
            provider,
            wallet: Arc::new(wallet),
            signer: Arc::new(signer),
        })
    }
}

// a better alternative to the built-in CachedNonceManager
// Benefits:
//
// 1. Lock-free
// We can do this because we don't need an additional address lookup
// and we can just use an atomic counter instead of a Mutex around u64 (odd that they do this btw)
//
// 2. No contention on first transaction
//
// Since we control when we create the provider to begin with, we don't need to wait for the
// first transaction to get the starting nonce. This prevents a race condition on creation
//
#[derive(Debug, Clone)]
pub struct FastNonceManager {
    // If set, will check the address against this
    address: Option<Address>,
    counter: Arc<AtomicU64>,
}

impl FastNonceManager {
    pub fn new(address: Option<Address>, first_nonce: u64) -> Self {
        Self {
            address,
            counter: Arc::new(AtomicU64::new(first_nonce)),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl NonceManager for FastNonceManager {
    async fn get_next_nonce<P, N>(&self, _provider: &P, address: Address) -> TransportResult<u64>
    where
        P: Provider<N>,
        N: Network,
    {
        if let Some(check_address) = self.address {
            if check_address != address {
                return Err(TransportErrorKind::custom_str(&format!(
                    "nonce manager address mismatch: expected {}, got {}",
                    check_address, address
                )));
            }
        }

        Ok(self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn preferred_transport() {
        // Not specified preference, websocket provided
        let transport = EvmClientBuilder::new(EvmClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: Some("bar".to_owned()),
            transport: None,
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EvmClientTransport::WebSocket));

        // Not specified preference, websocket not provided
        let transport = EvmClientBuilder::new(EvmClientConfig {
            ws_endpoint: None,
            http_endpoint: Some("bar".to_owned()),
            transport: None,
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EvmClientTransport::Http));

        // Specified Http preference, websocket provided
        let transport = EvmClientBuilder::new(EvmClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EvmClientTransport::Http),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EvmClientTransport::Http));

        // Specified Http preference, websocket not provided
        let transport = EvmClientBuilder::new(EvmClientConfig {
            ws_endpoint: None,
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EvmClientTransport::Http),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EvmClientTransport::Http));

        // Specified Websocket preference, websocket provided
        let transport = EvmClientBuilder::new(EvmClientConfig {
            ws_endpoint: Some("foo".to_owned()),
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EvmClientTransport::WebSocket),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EvmClientTransport::WebSocket));

        // Specified Websocket preference, websocket not provided
        let transport = EvmClientBuilder::new(EvmClientConfig {
            ws_endpoint: None,
            http_endpoint: Some("bar".to_owned()),
            transport: Some(EvmClientTransport::WebSocket),
            ..Default::default()
        })
        .preferred_transport();
        assert!(matches!(transport, EvmClientTransport::WebSocket));
    }
}
