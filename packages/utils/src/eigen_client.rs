pub mod config;
use std::sync::Arc;

use crate::{error::EthClientError, eth_client::EthSigningClient};
use alloy::{primitives::Address, providers::Provider, rpc::types::TransactionReceipt, sol};
use config::EigenClientConfig;
//use eigen_utils::delegationmanager::{DelegationManager::{self}, IDelegationManager::OperatorDetails};
use anyhow::{Result, Context};
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
    "../../contracts/abi/eigenlayer-middleware/DelegationManager.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EmptyContract,
    "../../contracts/abi/eigenlayer-middleware/EmptyContract.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    TransparentUpgradeableProxy,
    "../../contracts/abi/eigenlayer-middleware/TransparentUpgradeableProxy.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    ProxyAdmin,
    "../../contracts/abi/eigenlayer-middleware/ProxyAdmin.json"
);

impl EigenClient {
    pub fn new(eth: EthSigningClient, config: EigenClientConfig) -> Self {
        Self {
            eth,
            config: Arc::new(config),
        }
    }

    pub async fn deploy_delegation_manager(&self) -> Result<Address> {
        let admin = ProxyAdmin::deploy(self.eth.http_provider.clone()).await?.address().clone();

        let strategy_manager = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        let strategy_manager = TransparentUpgradeableProxy::deploy(self.eth.http_provider.clone(), strategy_manager, admin.clone(), b"".into()).await?.address().clone();
        let slasher = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        let slasher = TransparentUpgradeableProxy::deploy(self.eth.http_provider.clone(), slasher, admin.clone(), b"".into()).await?.address().clone();
        let pod_manager = EmptyContract::deploy(self.eth.http_provider.clone()).await?.address().clone();
        let pod_manager = TransparentUpgradeableProxy::deploy(self.eth.http_provider.clone(), pod_manager, admin.clone(), b"".into()).await?.address().clone();

        let res = DelegationManager::deploy(
            self.eth.http_provider.clone(), 
            strategy_manager, 
            slasher, 
            pod_manager, 
        ).await?;

        println!("{:?}", res);

        Ok(res.address().clone())

    }

    pub async fn register_operator(&self, delegation_manager_address: Option<Address>) -> Result<String> {
        let delegation_manager_address = delegation_manager_address.unwrap_or_else(|| self.config.core.addresses.delegation);
        let delegation_code = self
            .eth
            .http_provider
            .get_code_at(delegation_manager_address)
            .await?;


        if delegation_code.is_empty() {
            return Err(EthClientError::ContractNotDeployed(delegation_manager_address)).context("Eigenlayer delegation is not deployed")?;
        }

        let contract = DelegationManager::new(
            delegation_manager_address,
            self.eth.http_provider.clone(),
        );

        let operator = OperatorDetails {
            __deprecated_earningsReceiver: self.eth.address(),
            delegationApprover: self.eth.address(),
            stakerOptOutWindowBlocks: 0,
        };
        let contract_call = contract.registerAsOperator(operator, "".to_string());
        let binding_tx = contract_call.gas(300000).send().await?;

        let receipt: TransactionReceipt = dbg!(binding_tx.get_receipt().await?);

        let tx_status = receipt.status();
        if tx_status {
            Ok(receipt.transaction_hash.to_string())
        } else {
            Err(EthClientError::NoTransactionReceipt.into())
        }
    }
}
