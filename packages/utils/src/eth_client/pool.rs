use std::sync::atomic::AtomicU32;

use crate::config::EthereumChainConfig;
use alloy::{
    network::TransactionBuilder,
    primitives::{FixedBytes, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use anyhow::Result;
use deadpool::managed::{Manager, Metrics, RecycleResult};

use super::{EthClientBuilder, EthSigningClient};

// This will create a pool of signing clients, created on the fly as needed
// each client uses derivation path with an index starting at 1 and incrementing on creation
//
// when clients are first created in the pool, they are optionally funded `initial_client_wei` by the funder
// and this is kept behind a tokio mutex so this operation is only ever one-at-a-time
//
// however this is a one-time operation, so make sure the amount is large enough to cover the use-cases
// or call `fund` manually to top up the clients (which will have the same property of only one-at-a-time)
//
// to use the pool effectively, make sure you aren't using the same clients here anywhere else
// (including the funder)
//
// See deadpool docs for more details on how to use this pool

pub struct SigningClientPoolManager {
    mnemonic: String,
    chain_config: EthereumChainConfig,
    initial_client_wei: Option<U256>,
    derivation_index: AtomicU32,
    funder: tokio::sync::Mutex<EthSigningClient>,
    balance_maintainer: Option<BalanceMaintainer>,
}

#[derive(Clone, Debug)]
pub struct BalanceMaintainer {
    pub threshhold: U256,
    pub top_up_amount: U256,
}

impl BalanceMaintainer {
    pub fn new(threshhold: U256, top_up_amount: U256) -> Self {
        Self {
            threshhold,
            top_up_amount,
        }
    }
}

impl SigningClientPoolManager {
    pub fn new(
        funder: EthSigningClient,
        mnemonic: String,
        chain_config: EthereumChainConfig,
        initial_client_wei: Option<U256>,
        balance_maintainer: Option<BalanceMaintainer>,
    ) -> Self {
        Self {
            mnemonic,
            chain_config,
            derivation_index: AtomicU32::new(1),
            initial_client_wei,
            funder: tokio::sync::Mutex::new(funder),
            balance_maintainer
        }
    }

    async fn create_client(&self) -> Result<EthSigningClient> {
        let index = self
            .derivation_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let client_config =
            self.chain_config
                .to_client_config(Some(index), Some(self.mnemonic.clone()), None);

        let eth_client = EthClientBuilder::new(client_config).build_signing().await?;

        Ok(eth_client)
    }

    // sends wei to the address from the funder
    // returns the transaction hash
    async fn fund(&self, address: alloy::primitives::Address, wei: U256) -> Result<FixedBytes<32>> {
        let funder = self.funder.lock().await;

        let tx = TransactionRequest::default()
            .with_from(funder.address())
            .with_to(address)
            .with_value(wei);

        // Send the transaction and listen for the transaction to be included.
        let tx_hash = funder.provider.send_transaction(tx).await?.watch().await?;

        Ok(tx_hash)
    }

    async fn maintain_balance(
        &self,
        client: &EthSigningClient,
    ) -> Result<()> {
        if let Some(balance_maintainer) = &self.balance_maintainer {
            let balance = client.provider.get_balance(client.address()).await?;

            if balance < balance_maintainer.threshhold {
                let amount = balance_maintainer.top_up_amount - balance;
                self.fund(client.address(), amount).await?;
            }
        }

        Ok(())
    }
}

impl Manager for SigningClientPoolManager {
    type Type = EthSigningClient;
    type Error = anyhow::Error;

    async fn create(&self) -> Result<EthSigningClient> {
        let client = self.create_client().await?;

        if let Some(wei) = self.initial_client_wei {
            self.fund(client.address(), wei).await?;
        }

        Ok(client)
    }

    async fn recycle(
        &self,
        client: &mut EthSigningClient,
        metrics: &Metrics,
    ) -> RecycleResult<anyhow::Error> {
        tracing::debug!(
            "Pool recycled client {} {} times",
            client.address(),
            metrics.recycle_count
        );

        self.maintain_balance(client).await?;

        Ok(())
    }
}
