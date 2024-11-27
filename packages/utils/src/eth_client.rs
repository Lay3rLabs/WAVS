use alloy::{
    network::EthereumWallet,
    providers::{Identity, ProviderBuilder, RootProvider, WsConnect},
    pubsub::PubSubFrontend,
    signers::{
        k256::ecdsa::SigningKey,
        local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
    },
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct EthQueryClient {
    pub provider: RootProvider<PubSubFrontend>,
}

pub struct EthSigningClient {
    pub provider: RootProvider<PubSubFrontend>,
    /// The wallet is a collection of signers, with one designated as the default signer
    /// it allows signing transactions
    pub wallet: EthereumWallet,
    /// The signer is the same as the default signer in the wallet, but used for simple message signing
    /// due to type system limitations, we need to store it separately
    /// since the signer in `EthereumWallet` implements only `TxSigner`
    /// and there is not a direct way convert it into `Signer`
    pub signer: LocalSigner<SigningKey>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EthClientConfig {
    pub endpoint: String,
    pub mnemonic: Option<String>,
}

pub struct EthClientBuilder {
    pub config: EthClientConfig,
    pub provider_builder: ProviderBuilder<Identity, Identity>,
}

impl EthClientBuilder {
    pub fn new(config: EthClientConfig) -> Self {
        let provider_builder = ProviderBuilder::new();
        Self {
            config,
            provider_builder,
        }
    }

    pub async fn build_query(self) -> Result<EthQueryClient> {
        let ws = WsConnect::new(self.config.endpoint);

        let provider = self.provider_builder.on_ws(ws).await?;

        Ok(EthQueryClient { provider })
    }

    pub async fn build_signing(mut self) -> Result<EthSigningClient> {
        let mnemonic = self
            .config
            .mnemonic
            .take()
            .ok_or(EthClientError::MissingMnemonic)?;
        let provider = self.build_query().await?.provider;

        let signer = MnemonicBuilder::<English>::default()
            .phrase(mnemonic)
            .build()?;

        let wallet = signer.clone().into();

        Ok(EthSigningClient {
            provider,
            wallet,
            signer,
        })
    }
}

#[derive(Debug, Error)]
pub enum EthClientError {
    #[error("Missing mnemonic")]
    MissingMnemonic,
}
