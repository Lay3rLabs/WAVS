use std::{ops::Add, sync::Arc};

use crate::eigen_client::config::EigenClientConfig;
use crate::{error::EthClientError, eth_client::EthSigningClient};
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    rpc::types::TransactionReceipt,
    sol,
};
//use eigen_utils::delegationmanager::{DelegationManager::{self}, IDelegationManager::OperatorDetails};
use super::*;
use anyhow::{Context, Result};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    AvsDirectory,
    "../../out/AvsDirectory.sol/AvsDirectory.json"
);

impl EigenClient {
    pub async fn deploy_core_contracts(&self) -> Result<()> {
        //let pauser_registry = PauserRegistry::deploy(self.eth.http_provider.clone(), vec![], admin_address).await?;
        // let strategy_manager = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        // let strategy_manager = TransparentUpgradeableProxy::new(strategy_manager, self.eth.http_provider.clone());
        // let upgrade_call =
        // proxy_admin.upgradeAndCall(proxy, implementation, data)
        // let slasher = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        // let slasher = TransparentUpgradeableProxy::deploy(self.eth.http_provider.clone(), slasher, admin_address.clone(), b"".into()).await?.address().clone();
        // let pod_manager = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        // let pod_manager = TransparentUpgradeableProxy::deploy(self.eth.http_provider.clone(), pod_manager, admin_address.clone(), b"".into()).await?.address().clone();

        println!("BP 1");

        let proxies = ProxyAddresses::new(&self.eth).await?;

        //let delegation_manager  = DelegationManager::new(proxies.delegation_manager, self.eth.http_provider.clone());

        let delegation_manager = DelegationManager::deploy(
            self.eth.http_provider.clone(),
            proxies.strategy_manager.clone(),
            Address::ZERO,
            proxies.eigen_pod_manager,
        )
        .await?;

        println!("BP 2");

        Ok(())
    }
}

struct ProxyAddresses {
    pub admin: Address,
    pub delegation_manager: Address,
    pub avs_directory: Address,
    pub strategy_manager: Address,
    pub eigen_pod_manager: Address,
    pub rewards_coordinator: Address,
    pub eigen_pod_beacon: Address,
    pub pauser_registery: Address,
    pub strategy_factory: Address,
}

impl ProxyAddresses {
    pub async fn new(eth: &EthSigningClient) -> Result<Self> {
        async fn setup_empty_proxy(
            eth: &EthSigningClient,
            proxy_admin: Address,
        ) -> Result<Address> {
            let empty_contract = EmptyContract::deploy(eth.http_provider.clone()).await?;
            let empty_contract_address = empty_contract.address().clone();
            let proxy = TransparentUpgradeableProxy::deploy(
                eth.http_provider.clone(),
                empty_contract_address,
                proxy_admin.clone(),
                b"".into(),
            )
            .await?;
            Ok(proxy.address().clone())
        }

        let admin = ProxyAdmin::deploy(eth.http_provider.clone()).await?;
        let admin_address = admin.address().clone();

        Ok(Self {
            admin: admin_address.clone(),
            delegation_manager: setup_empty_proxy(eth, admin_address).await?,
            avs_directory: setup_empty_proxy(eth, admin_address).await?,
            strategy_manager: setup_empty_proxy(eth, admin_address).await?,
            eigen_pod_manager: setup_empty_proxy(eth, admin_address).await?,
            rewards_coordinator: setup_empty_proxy(eth, admin_address).await?,
            eigen_pod_beacon: setup_empty_proxy(eth, admin_address).await?,
            pauser_registery: setup_empty_proxy(eth, admin_address).await?,
            strategy_factory: setup_empty_proxy(eth, admin_address).await?,
        })
    }
}
