pub mod config;
pub mod temp_ext;
use std::sync::Arc;
use temp_ext::*;

use crate::{error::EthClientError, eth_client::EthSigningClient};
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    rpc::types::TransactionReceipt,
    sol,
};
use config::EigenClientConfig;
//use eigen_utils::delegationmanager::{DelegationManager::{self}, IDelegationManager::OperatorDetails};
use anyhow::{Context, Result};
use IDelegationManager::OperatorDetails;

#[derive(Clone)]
pub struct EigenClient {
    pub eth: EthSigningClient,
    pub config: Arc<EigenClientConfig>,
}

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    DelegationManager,
    "../../out/DelegationManager.sol/DelegationManager.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EmptyContract,
    "../../out/EmptyContract.sol/EmptyContract.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TransparentUpgradeableProxy,
    "../../out/TransparentUpgradeableProxy.sol/TransparentUpgradeableProxy.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    ProxyAdmin,
    "../../out/ProxyAdmin.sol/ProxyAdmin.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    PauserRegistry,
    "../../out/PauserRegistry.sol/PauserRegistry.json"
);

impl EigenClient {
    pub fn new(eth: EthSigningClient, config: EigenClientConfig) -> Self {
        Self {
            eth,
            config: Arc::new(config),
        }
    }

    pub async fn deploy_delegation_manager(&self) -> Result<Address> {
        let proxy_admin = ProxyAdmin::deploy(self.eth.http_provider.clone()).await?;
        let admin_address = proxy_admin.address().clone();

        let pauser_registry =
            PauserRegistry::deploy(self.eth.http_provider.clone(), vec![], admin_address).await?;
        // let strategy_manager = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        // let strategy_manager = TransparentUpgradeableProxy::new(strategy_manager, self.eth.http_provider.clone());
        // let upgrade_call =
        // proxy_admin.upgradeAndCall(proxy, implementation, data)
        // let slasher = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        // let slasher = TransparentUpgradeableProxy::deploy(self.eth.http_provider.clone(), slasher, admin_address.clone(), b"".into()).await?.address().clone();
        // let pod_manager = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        // let pod_manager = TransparentUpgradeableProxy::deploy(self.eth.http_provider.clone(), pod_manager, admin_address.clone(), b"".into()).await?.address().clone();

        let delegation_manager = EmptyContract::deploy(self.eth.http_provider.clone())
            .await?
            .address()
            .clone();
        let delegation_manager =
            DelegationManager::new(delegation_manager, self.eth.http_provider.clone());

        // let call = delegation_manager.initialize(admin_address, pauser_registry.address().clone(), U256::ZERO, U256::ZERO, vec![], vec![]).await?;
        // let res = proxy_admin.upgradeAndCall(proxy_admin, delegation_manager, call).send().await?.get_receipt().await?;

        // println!("{:?}", res);

        Ok(delegation_manager.address().clone())
    }

    pub async fn register_operator(
        &self,
        delegation_manager_address: Option<Address>,
    ) -> Result<String> {
        let delegation_manager_address =
            delegation_manager_address.unwrap_or_else(|| self.config.core.addresses.delegation);
        let delegation_code = self
            .eth
            .http_provider
            .get_code_at(delegation_manager_address)
            .await?;

        if delegation_code.is_empty() {
            return Err(EthClientError::ContractNotDeployed(
                delegation_manager_address,
            ))
            .context("Eigenlayer delegation is not deployed")?;
        }

        let contract =
            DelegationManager::new(delegation_manager_address, self.eth.http_provider.clone());

        let operator = OperatorDetails {
            __deprecated_earningsReceiver: self.eth.address(),
            delegationApprover: self.eth.address(),
            stakerOptOutWindowBlocks: 0,
        };
        let contract_call = contract.registerAsOperator(operator, "".to_string());
        let binding_tx = contract_call.gas(300000).send().await?;

        let receipt: TransactionReceipt = binding_tx.get_receipt().await?;

        let tx_status = receipt.status();
        if tx_status {
            Ok(receipt.transaction_hash.to_string())
        } else {
            Err(EthClientError::NoTransactionReceipt.into())
        }
    }
}
