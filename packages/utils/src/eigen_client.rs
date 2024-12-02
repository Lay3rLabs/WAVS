pub mod config;
use std::sync::Arc;

use crate::{error::EthClientError, eth_client::EthSigningClient};
use alloy::{rpc::types::TransactionReceipt, sol};
use config::EigenClientConfig;
//use eigen_utils::delegationmanager::{DelegationManager::{self}, IDelegationManager::OperatorDetails};
use anyhow::Result;
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

impl EigenClient {
    pub fn new(eth: EthSigningClient, config: EigenClientConfig) -> Self {
        Self {
            eth,
            config: Arc::new(config),
        }
    }

    pub async fn register_operator(&self) -> Result<String> {
        let contract = DelegationManager::new(
            self.config.core.addresses.delegation,
            self.eth.http_provider.clone(),
        );

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
