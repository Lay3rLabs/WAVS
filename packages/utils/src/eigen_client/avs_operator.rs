use alloy::{
    contract::{ContractInstance, Interface},
    network::NetworkWallet,
    primitives::{keccak256, Address},
    providers::{Provider, WalletProvider},
    rpc::types::{TransactionReceipt, TransactionRequest},
    signers::Signer,
    sol_types::SolValue,
};

use crate::{
    error::EthClientError,
    hello_world::solidity_types::hello_world::HelloWorldServiceManager::NewTaskCreated,
};

use super::{
    config::CoreAVSAddresses,
    solidity_types::delegation_manager::{DelegationManager, IDelegationManager::OperatorDetails},
    EigenClient,
};
use anyhow::{Context, Result};

impl EigenClient {
    pub async fn register_operator(&self, avs_addresses: &CoreAVSAddresses) -> Result<String> {
        let delegation_manager_address = avs_addresses.delegation_manager.clone();
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

    pub async fn sign_and_respond_to_task(
        &self,
        avs_address: Address,
        new_task_event: NewTaskCreated,
    ) -> Result<String> {
        let message = format!("Hello, {}", new_task_event.task.name);
        // Check this if 2 different strings
        let message_hash = keccak256(message);
        // Check if this is the same as toEthSignedMessageHash
        let message_bytes = message_hash.as_slice();
        // TODO: Sign hash or sign message?
        let signature = self.eth.signer.sign_message(message_bytes).await?;
        tracing::debug!(
            "Signing and responding to task {}",
            new_task_event.taskIndex
        );
        let operators = vec![self.eth.signer.address()];
        let signatures = vec![signature.as_bytes().to_vec()];

        // TODO: what type are we supposed to use?
        use crate::hello_world::solidity_types::hello_world::HelloWorldServiceManager::respondToTaskCall;
        // TODO: What is this for
        let reference_block = self.eth.ws_provider.get_block_number().await? - 1;

        let signed_task = (operators, signatures, reference_block).abi_encode();
        let respond_to_task_call = respondToTaskCall {
            task: new_task_event.task,
            referenceTaskIndex: new_task_event.taskIndex,
            signature: signed_task.into(),
        };
        // Send it to avs

        todo!()
    }
}
