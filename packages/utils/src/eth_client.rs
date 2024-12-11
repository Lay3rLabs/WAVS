use std::sync::Arc;

use alloy::{
    network::EthereumWallet,
    primitives::{Address, TxHash},
    providers::{Identity, ProviderBuilder, RootProvider, WsConnect},
    pubsub::PubSubFrontend,
    signers::{
        k256::ecdsa::SigningKey,
        local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
    },
    transports::http::{Client, Http},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    eigen_client::solidity_types::{HttpSigningProvider, WsSigningProvider},
    error::EthClientError,
};

#[derive(Clone)]
pub struct EthQueryClient {
    pub config: EthClientConfig,
    pub ws_provider: RootProvider<PubSubFrontend>,
    pub http_provider: RootProvider<Http<Client>>,
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
    pub ws_provider: WsSigningProvider,
    pub http_provider: HttpSigningProvider,
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
    pub ws_endpoint: String,
    pub http_endpoint: String,
    pub mnemonic: Option<String>,
    pub hd_index: Option<u32>,
}

pub struct EthClientBuilder {
    pub config: EthClientConfig,
    pub ws_provider_builder: ProviderBuilder<Identity, Identity>,
    pub http_provider_builder: ProviderBuilder<Identity, Identity>,
}

impl EthClientBuilder {
    pub fn new(config: EthClientConfig) -> Self {
        let ws_provider_builder = ProviderBuilder::new();
        let http_provider_builder = ProviderBuilder::new();
        Self {
            config,
            ws_provider_builder,
            http_provider_builder,
        }
    }

    pub async fn build_query(self) -> Result<EthQueryClient> {
        let ws = WsConnect::new(&self.config.ws_endpoint);
        let ws_provider = self.ws_provider_builder.on_ws(ws).await?;

        let http_provider = self
            .http_provider_builder
            .on_http(self.config.http_endpoint.parse()?);

        Ok(EthQueryClient {
            config: self.config,
            ws_provider,
            http_provider,
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

        let ws = WsConnect::new(&self.config.ws_endpoint);
        let ws_provider = self
            .ws_provider_builder
            .with_recommended_fillers()
            .wallet(wallet.clone())
            .on_ws(ws)
            .await?;

        let http_provider = self
            .http_provider_builder
            .with_recommended_fillers()
            .wallet(wallet.clone())
            .on_http(self.config.http_endpoint.parse()?);

        Ok(EthSigningClient {
            config: self.config,
            ws_provider,
            http_provider,
            wallet: Arc::new(wallet),
            signer: Arc::new(signer),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddTaskRequest {
    pub task_id: String,
    /// Address of the avs
    pub service: Address,
    pub operator: Address,
    pub new_data: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Returns hash in case there if threshold reached
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddTaskResponse {
    pub hash: Option<TxHash>,
}
