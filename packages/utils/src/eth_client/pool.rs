use std::{ops::Deref, sync::atomic::AtomicU32};

use crate::config::EthereumChainConfig;
use alloy::{
    network::TransactionBuilder,
    primitives::{FixedBytes, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use anyhow::{anyhow, Context, Result};
use deadpool::managed::{Manager, Metrics, Object, Pool, RecycleResult};
use serde::{Deserialize, Serialize};

use super::{EthClientBuilder, EthSigningClient};

// This will create a pool of signing clients, created on the fly as needed
// each client uses derivation path with an index starting at 1 and incrementing on creation
//
// when clients are first created in the pool, they are optionally funded `initial_client_wei` by the funder
// and as clients are recycled, their balance is also optionally maintained  (see `BalanceMaintainer`)
//
// to use the pool effectively, make sure you aren't using the same clients here anywhere else
//
// In order to prevent misuse of the pool, we only expose wrapper types
// this also makes it a bit easier to use since they don't need to import deadpool
pub struct EthSigningClientFromPool(Object<EthSigningClientPoolManager>);

type EthFundingClient = EthSigningClient;

impl Deref for EthSigningClientFromPool {
    type Target = EthSigningClient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct EthSigningClientPoolBuilder {
    // This can be:
    // - a mnemonic
    // - a private key
    // - None, which will use index 0 of client_mnemonic (clients always start at 1)
    pub funder_mnemonic_or_key: Option<String>,
    pub client_mnemonic: String, // must be a mnemonic
    pub chain_config: EthereumChainConfig,
    // default is 16
    pub max_size: Option<usize>,
    // not required
    pub initial_client_wei: Option<U256>,
    // not required
    pub balance_maintainer: Option<BalanceMaintainer>,
}

impl EthSigningClientPoolBuilder {
    pub fn new(
        funder_mnemonic_or_key: Option<String>,
        client_mnemonic: String,
        chain_config: EthereumChainConfig,
    ) -> Self {
        Self {
            funder_mnemonic_or_key,
            client_mnemonic,
            chain_config,
            max_size: None,
            initial_client_wei: None,
            balance_maintainer: None,
        }
    }

    pub fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_size = Some(max_size);
        self
    }
    pub fn with_initial_client_wei(mut self, initial_client_wei: U256) -> Self {
        self.initial_client_wei = Some(initial_client_wei);
        self
    }
    pub fn with_balance_maintainer(mut self, balance_maintainer: BalanceMaintainer) -> Self {
        self.balance_maintainer = Some(balance_maintainer);
        self
    }

    pub async fn build(self) -> Result<EthSigningClientPool> {
        let Self {
            funder_mnemonic_or_key,
            client_mnemonic,
            chain_config,
            max_size,
            initial_client_wei,
            balance_maintainer,
        } = self;

        // If balance_maintainer exists, validate that top_up_amount > 0
        if let Some(maintainer) = &balance_maintainer {
            if maintainer.top_up_amount.is_zero() {
                return Err(anyhow::anyhow!(
                    "Balance maintainer top_up_amount must be greater than zero"
                ));
            }
        }

        let funder_config = chain_config.to_client_config(
            None,
            Some(funder_mnemonic_or_key.unwrap_or_else(|| client_mnemonic.clone())),
            None,
        );

        let funder = EthClientBuilder::new(funder_config).build_signing().await?;

        //let funder = Arc::new(tokio::sync::Mutex::new(funder));

        let manager = EthSigningClientPoolManager::new(
            funder.clone(),
            client_mnemonic,
            chain_config,
            initial_client_wei,
        )?;

        let pool = Pool::builder(manager)
            .max_size(max_size.unwrap_or(16))
            .build()
            .context("Failed to create signing client pool")?;

        Ok(EthSigningClientPool {
            inner: pool,
            funder,
            balance_maintainer,
        })
    }
}

#[derive(Clone)]
pub struct EthSigningClientPool {
    inner: Pool<EthSigningClientPoolManager>,
    funder: EthFundingClient,
    balance_maintainer: Option<BalanceMaintainer>,
}

impl EthSigningClientPool {
    pub async fn get(&self) -> Result<EthSigningClientFromPool> {
        let client = self.inner.get().await.map_err(|e| anyhow!("{e:?}"))?;

        // If balance maintainer is set, check and maintain balance
        if let Some(balance_maintainer) = &self.balance_maintainer {
            let balance = client.provider.get_balance(client.address()).await?;

            if balance < balance_maintainer.threshhold {
                // Balance maintainer was already validated at creation, so we know top_up_amount > balance
                let amount = balance_maintainer.top_up_amount - balance;
                //fund(&*self.funder.lock().await, client.address(), amount).await?;
                fund(&self.funder, client.address(), amount).await?;
            }
        }

        Ok(EthSigningClientFromPool(client))
    }
}

struct EthSigningClientPoolManager {
    mnemonic: String,
    chain_config: EthereumChainConfig,
    initial_client_wei: Option<U256>,
    derivation_index: AtomicU32,
    funder: EthFundingClient,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BalanceMaintainer {
    threshhold: U256,
    top_up_amount: U256,
}

impl BalanceMaintainer {
    pub fn new(threshhold: U256, top_up_amount: U256) -> Result<Self> {
        // Ensure top_up_amount is greater than threshhold
        if top_up_amount <= threshhold {
            return Err(anyhow::anyhow!(
                "Balance maintainer top_up_amount ({}) must be greater than threshhold ({})",
                top_up_amount,
                threshhold
            ));
        }

        Ok(Self {
            threshhold,
            top_up_amount,
        })
    }
}

impl EthSigningClientPoolManager {
    pub fn new(
        funder: EthFundingClient,
        mnemonic: String,
        chain_config: EthereumChainConfig,
        initial_client_wei: Option<U256>,
    ) -> Result<Self> {
        Ok(Self {
            funder,
            mnemonic,
            chain_config,
            derivation_index: AtomicU32::new(1),
            initial_client_wei,
        })
    }

    async fn create_client(&self) -> Result<EthSigningClient> {
        let index = self
            .derivation_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let client_config =
            self.chain_config
                .to_client_config(Some(index), Some(self.mnemonic.clone()), None);

        let client = EthClientBuilder::new(client_config).build_signing().await?;

        if let Some(amount) = self.initial_client_wei {
            fund(&self.funder, client.address(), amount).await?;
        }

        Ok(client)
    }
}

impl Manager for EthSigningClientPoolManager {
    type Type = EthSigningClient;
    type Error = anyhow::Error;

    async fn create(&self) -> Result<EthSigningClient> {
        self.create_client().await
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

        Ok(())
    }
}

// sends wei to the address from the funder
// returns the transaction hash
async fn fund(
    funder: &EthSigningClient,
    address: alloy::primitives::Address,
    wei: U256,
) -> Result<FixedBytes<32>> {
    let tx = TransactionRequest::default()
        .with_from(funder.address())
        .with_to(address)
        .with_value(wei);

    // Send the transaction and listen for the transaction to be included.
    let tx_hash = funder.provider.send_transaction(tx).await?.watch().await?;

    Ok(tx_hash)
}
