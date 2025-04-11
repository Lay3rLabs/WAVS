pub mod contracts;
pub mod pool;
pub mod signing;

use alloy::{
    hex,
    network::{EthereumWallet, Network},
    primitives::Address,
    providers::{
        fillers::{BlobGasFiller, ChainIdFiller, GasFiller, NonceManager},
        DynProvider, Provider, ProviderBuilder,
    },
    signers::{
        k256::{ecdsa::SigningKey, SecretKey},
        local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
    },
    transports::{TransportErrorKind, TransportResult},
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{atomic::AtomicU64, Arc};

use crate::error::EthClientError;

#[derive(Clone)]
pub struct EthQueryClient {
    pub config: EthClientConfig,
    pub provider: DynProvider,
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
    pub provider: DynProvider,
    /// The wallet is a collection of signers, with one designated as the default signer
    /// it allows signing transactions
    pub wallet: Arc<EthereumWallet>,
    /// The signer is the same as the default signer in the wallet, but used for simple message signing
    /// due to type system limitations, we need to store it separately
    /// since the signer in `EthereumWallet` implements only `TxSigner`
    /// and there is not a direct way convert it into `Signer`
    pub signer: Arc<LocalSigner<SigningKey>>,
    // if the client is configured with `use_fast_nonce_manager`, this will be set
    pub nonce_manager: Option<FastNonceManager>,
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
            provider: DynProvider::new(ProviderBuilder::new().on_builtin(&endpoint).await?),
        })
    }

    pub async fn build_signing(mut self, use_fast_nonce_manager: bool) -> Result<EthSigningClient> {
        let mnemonic = self
            .config
            .mnemonic
            .take()
            .ok_or(EthClientError::MissingMnemonic)?;

        let hd_index = self.config.hd_index.unwrap_or(0);

        let signer: LocalSigner<SigningKey> = if let Some(stripped) = mnemonic.strip_prefix("0x") {
            if hd_index > 0 {
                return Err(EthClientError::DerivationWithPrivateKey.into());
            }
            let private_key = hex::decode(stripped)?;
            let secret_key = SecretKey::from_slice(&private_key)?;
            LocalSigner::from_signing_key(secret_key.into())
        } else {
            MnemonicBuilder::<English>::default()
                .phrase(mnemonic)
                .index(hd_index)?
                .build()?
        };

        let wallet: EthereumWallet = signer.clone().into();

        let endpoint = self.endpoint()?;

        let nonce_manager = if use_fast_nonce_manager {
            let query_provider = ProviderBuilder::new().on_builtin(&endpoint).await?;
            let first_nonce = query_provider
                .get_transaction_count(signer.address())
                .await?;

            Some(FastNonceManager::new(Some(signer.address()), first_nonce))
        } else {
            None
        };

        let provider = match nonce_manager.clone() {
            Some(nonce_manager) => DynProvider::new(
                ProviderBuilder::default()
                    .with_nonce_management(nonce_manager)
                    .filler(GasFiller)
                    .filler(BlobGasFiller)
                    .filler(ChainIdFiller::new(None))
                    .wallet(wallet.clone())
                    .on_builtin(&endpoint)
                    .await?,
            ),
            None => DynProvider::new(
                ProviderBuilder::new()
                    .wallet(wallet.clone())
                    .on_builtin(&endpoint)
                    .await?,
            ),
        };

        Ok(EthSigningClient {
            config: self.config,
            provider,
            wallet: Arc::new(wallet),
            signer: Arc::new(signer),
            nonce_manager,
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
