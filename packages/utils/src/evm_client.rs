pub mod contracts;
pub mod signing;

use alloy_network::{EthereumWallet, Network, TransactionBuilder};
use alloy_primitives::{utils::parse_ether, Address};
use alloy_provider::{
    fillers::{BlobGasFiller, ChainIdFiller, GasFiller, NonceManager},
    DynProvider, Provider, ProviderBuilder, WsConnect,
};
use alloy_rpc_types_eth::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use alloy_transport::{TransportErrorKind, TransportResult};
use anyhow::Result;
use async_trait::async_trait;
use signing::make_signer;
use std::{
    str::FromStr,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};
use wavs_types::Credential;

use crate::error::EvmClientError;

#[derive(Clone)]
pub struct EvmQueryClient {
    pub endpoint: EvmEndpoint,
    pub provider: DynProvider,
}

#[derive(Debug, Clone)]
pub enum EvmEndpoint {
    WebSocket(reqwest::Url),
    Http(reqwest::Url),
}

impl FromStr for EvmEndpoint {
    type Err = EvmClientError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url =
            reqwest::Url::parse(s).map_err(|e| EvmClientError::ParseEndpoint(e.to_string()))?;
        match url.scheme() {
            "ws" | "wss" => Ok(EvmEndpoint::WebSocket(url)),
            "http" | "https" => Ok(EvmEndpoint::Http(url)),
            scheme => Err(EvmClientError::ParseEndpoint(format!(
                "could not determine endpoint from scheme {scheme} (full url: {s})"
            ))),
        }
    }
}

impl std::fmt::Display for EvmEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvmEndpoint::WebSocket(url) => write!(f, "{}", url),
            EvmEndpoint::Http(url) => write!(f, "{}", url),
        }
    }
}

impl EvmEndpoint {
    pub fn new_http(url: &str) -> Result<Self, EvmClientError> {
        url.parse::<Self>().and_then(|endpoint| {
            if matches!(endpoint, EvmEndpoint::Http(_)) {
                Ok(endpoint)
            } else {
                Err(EvmClientError::ParseEndpoint(
                    "url scheme is not http or https".to_string(),
                ))
            }
        })
    }
    pub fn new_ws(url: &str) -> Result<Self, EvmClientError> {
        url.parse::<Self>().and_then(|endpoint| {
            if matches!(endpoint, EvmEndpoint::WebSocket(_)) {
                Ok(endpoint)
            } else {
                Err(EvmClientError::ParseEndpoint(
                    "url scheme is not ws or wss".to_string(),
                ))
            }
        })
    }

    pub async fn to_provider(&self) -> std::result::Result<DynProvider, EvmClientError> {
        Ok(match self {
            EvmEndpoint::WebSocket(url) => {
                let ws = WsConnect::new(url.clone());
                DynProvider::new(
                    ProviderBuilder::new()
                        .connect_ws(ws)
                        .await
                        .map_err(|e| EvmClientError::WebSocketProvider(e.into()))?,
                )
            }
            EvmEndpoint::Http(url) => {
                DynProvider::new(ProviderBuilder::new().connect_http(url.clone()))
            }
        })
    }
}

impl EvmQueryClient {
    pub async fn new(endpoint: EvmEndpoint) -> std::result::Result<Self, EvmClientError> {
        Ok(EvmQueryClient {
            provider: endpoint.to_provider().await?,
            endpoint,
        })
    }
}

impl std::fmt::Debug for EvmQueryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmQueryClient")
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

#[derive(Clone)]
pub struct EvmSigningClient {
    pub config: EvmSigningClientConfig,
    pub provider: DynProvider,
    /// The wallet is a collection of signers, with one designated as the default signer
    /// it allows signing transactions
    pub wallet: Arc<EthereumWallet>,
    /// The signer is the same as the default signer in the wallet, but used for simple message signing
    /// due to type system limitations, we need to store it separately
    /// since the signer in `EthereumWallet` implements only `TxSigner`
    /// and there is not a direct way convert it into `Signer`
    pub signer: Arc<PrivateKeySigner>,
    pub nonce_manager: AnyNonceManager,
}

impl EvmSigningClient {
    pub async fn transfer_funds(&self, dest_addr: Address, amount_eth: &str) -> Result<()> {
        let tx = TransactionRequest::default()
            .with_from(self.address())
            .with_to(dest_addr)
            .with_value(parse_ether(amount_eth)?);

        self.provider.send_transaction(tx).await?.watch().await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EvmSigningClientConfig {
    pub endpoint: EvmEndpoint,
    pub credential: Credential,
    pub hd_index: Option<u32>,
    /// If a transaction does not have `max_gas` set, then it will estimate
    /// however the actual gas needed fluctuates, so we can pad it with a multiplier
    /// if unset, it will be 1.25
    pub gas_estimate_multiplier: Option<f32>,
    /// The interval at which to poll the provider for new blocks
    /// if unset, will use the default of the provider (which may differ across networks)
    pub poll_interval: Option<Duration>,
    pub nonce_manager_kind: NonceManagerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceManagerKind {
    Fast,
    Safe,
}

#[derive(Debug, Clone)]
pub enum AnyNonceManager {
    Fast(FastNonceManager),
    Safe(SafeNonceManager),
}
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl NonceManager for AnyNonceManager {
    async fn get_next_nonce<P, N>(&self, _provider: &P, address: Address) -> TransportResult<u64>
    where
        P: Provider<N>,
        N: Network,
    {
        match self {
            AnyNonceManager::Fast(nonce_manager) => {
                nonce_manager.get_next_nonce(_provider, address).await
            }
            AnyNonceManager::Safe(nonce_manager) => {
                nonce_manager.get_next_nonce(_provider, address).await
            }
        }
    }
}

impl EvmSigningClientConfig {
    pub fn new(endpoint: EvmEndpoint, credential: Credential) -> Self {
        Self {
            endpoint,
            credential,
            hd_index: None,
            gas_estimate_multiplier: None,
            poll_interval: None,
            nonce_manager_kind: NonceManagerKind::Fast,
        }
    }

    pub fn new_anvil(endpoint_url: &str) -> Self {
        Self {
            endpoint: EvmEndpoint::from_str(endpoint_url).expect("Failed to parse anvil endpoint"),
            credential: Credential::new(
                "test test test test test test test test test test test junk".to_string(),
            ),
            hd_index: None,
            gas_estimate_multiplier: None,
            poll_interval: None,
            nonce_manager_kind: NonceManagerKind::Fast,
        }
    }

    pub fn with_hd_index(mut self, hd_index: u32) -> Self {
        self.hd_index = Some(hd_index);
        self
    }
    pub fn with_gas_estimate_multiplier(mut self, gas_estimate_multiplier: f32) -> Self {
        self.gas_estimate_multiplier = Some(gas_estimate_multiplier);
        self
    }
}

impl EvmSigningClient {
    const DEFAULT_GAS_ESTIMATE_MULTIPLIER: f32 = 1.25;

    pub async fn new(config: EvmSigningClientConfig) -> Result<Self> {
        let signer = make_signer(&config.credential, config.hd_index)?;

        let wallet: EthereumWallet = signer.clone().into();

        let nonce_manager = match config.nonce_manager_kind {
            NonceManagerKind::Fast => {
                let nonce_manager = FastNonceManager::new(Some(signer.address()));
                nonce_manager
                    .set_current_nonce(&config.endpoint.to_provider().await?)
                    .await?;
                AnyNonceManager::Fast(nonce_manager)
            }
            NonceManagerKind::Safe => {
                let nonce_manager = SafeNonceManager::new(Some(signer.address()));
                AnyNonceManager::Safe(nonce_manager)
            }
        };

        let builder = ProviderBuilder::default()
            .with_nonce_management(nonce_manager.clone())
            .filler(GasFiller)
            .filler(BlobGasFiller)
            .filler(ChainIdFiller::new(None))
            .wallet(wallet.clone());

        let provider = match &config.endpoint {
            EvmEndpoint::WebSocket(url) => {
                let ws = WsConnect::new(url.clone());
                DynProvider::new(builder.connect_ws(ws).await?)
            }
            EvmEndpoint::Http(url) => DynProvider::new(builder.connect_http(url.clone())),
        };

        if let Some(poll_interval) = config.poll_interval {
            provider.client().set_poll_interval(poll_interval);
        }

        Ok(Self {
            config,
            provider,
            nonce_manager,
            wallet: Arc::new(wallet),
            signer: Arc::new(signer),
        })
    }

    pub fn gas_estimate_multiplier(&self) -> f32 {
        self.config
            .gas_estimate_multiplier
            .unwrap_or(Self::DEFAULT_GAS_ESTIMATE_MULTIPLIER)
    }

    pub async fn new_anvil(endpoint_url: &str) -> Result<Self> {
        Self::new(EvmSigningClientConfig::new_anvil(endpoint_url)).await
    }
}

impl std::fmt::Debug for EvmSigningClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmSigningClient")
            .field("endpoint", &self.config.endpoint)
            .field("address", &self.address())
            .finish()
    }
}

impl EvmSigningClient {
    pub fn address(&self) -> Address {
        self.signer.address()
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
    pub fn new(address: Option<Address>) -> Self {
        Self {
            address,
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn set_current_nonce<P, N>(&self, provider: &P) -> anyhow::Result<u64>
    where
        P: Provider<N>,
        N: Network,
    {
        match self.address {
            Some(address) => {
                let current_nonce = provider.get_transaction_count(address).await?;
                self.counter
                    .store(current_nonce, std::sync::atomic::Ordering::SeqCst);
                Ok(current_nonce)
            }
            None => Err(anyhow::anyhow!(
                "FastNonceManager address is not set, cannot set current nonce"
            )),
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
                    "nonce manager address mismatch: expected {check_address}, got {address}"
                )));
            }
        }

        Ok(self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }
}

#[derive(Debug, Clone)]
pub struct SafeNonceManager {
    // If set, will check the address against this
    address: Option<Address>,
}

impl SafeNonceManager {
    pub fn new(address: Option<Address>) -> Self {
        Self { address }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl NonceManager for SafeNonceManager {
    async fn get_next_nonce<P, N>(&self, provider: &P, address: Address) -> TransportResult<u64>
    where
        P: Provider<N>,
        N: Network,
    {
        if let Some(check_address) = self.address {
            if check_address != address {
                return Err(TransportErrorKind::custom_str(&format!(
                    "nonce manager address mismatch: expected {check_address}, got {address}"
                )));
            }
        }
        Ok(provider.get_transaction_count(address).await?)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_endpoint() {
        let endpoint = EvmEndpoint::from_str("ws://localhost:8545").unwrap();
        assert!(matches!(endpoint, EvmEndpoint::WebSocket(_)));

        let endpoint = EvmEndpoint::from_str("http://localhost:8545").unwrap();
        assert!(matches!(endpoint, EvmEndpoint::Http(_)));

        let endpoint = EvmEndpoint::from_str("https://localhost:8545").unwrap();
        assert!(matches!(endpoint, EvmEndpoint::Http(_)));

        let endpoint = EvmEndpoint::from_str("wss://localhost:8545").unwrap();
        assert!(matches!(endpoint, EvmEndpoint::WebSocket(_)));

        let endpoint = EvmEndpoint::from_str("localhost:8545").unwrap_err();
        assert!(matches!(endpoint, EvmClientError::ParseEndpoint(_)));
    }
}
