pub mod config;
use std::sync::Arc;

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

    pub async fn deploy_delegation_manager(&self, url: String) -> Result<Address> {
        // TODO: hardcoded
        let http_provider = alloy::providers::ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(self.eth.wallet.clone())
            .on_http(url.parse().unwrap());
        let proxy_admin = ProxyAdmin::deploy(http_provider.clone()).await?;
        let admin_address = proxy_admin.address().clone();

        let pauser_registry =
            PauserRegistry::deploy(http_provider.clone(), vec![], admin_address).await?;
        let delegation_manager = EmptyContract::deploy(http_provider.clone())
            .await?
            .address()
            .clone();
        let delegation_manager = TransparentUpgradeableProxy::deploy(
            http_provider.clone(),
            delegation_manager,
            admin_address.clone(),
            Default::default(),
        )
        .await?;
        let delegation_manager =
            DelegationManager::new(delegation_manager.address().clone(), http_provider.clone());
        let call = dbg!(delegation_manager
            .initialize(
                admin_address.clone(),
                pauser_registry.address().clone(),
                U256::ZERO,
                U256::ZERO,
                vec![],
                vec![]
            )
            .into_transaction_request());
        let upgrade_and_call = proxy_admin.upgradeAndCall(
            admin_address,
            delegation_manager.address().clone(),
            call.input.input.unwrap(),
        );
        let _call = upgrade_and_call.call().await?;
        let res = upgrade_and_call.send().await?.get_receipt().await?;

        println!("{:?}", res);

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
