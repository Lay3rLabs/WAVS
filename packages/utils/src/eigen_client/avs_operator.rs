use alloy::{providers::Provider, rpc::types::TransactionReceipt};

use crate::error::EthClientError;

use super::{
    config::CoreAVSAddresses,
    solidity_types::delegation_manager::{DelegationManager, IDelegationManager::OperatorDetails},
    EigenClient,
};
use anyhow::{Context, Result};

impl EigenClient {
    pub async fn register_operator(&self, avs_addresses: &CoreAVSAddresses) -> Result<String> {
        let delegation_manager_address = avs_addresses.delegation_manager;
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
