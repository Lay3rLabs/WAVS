//! Hello world client implementations
//!

use crate::{alloy_helpers::SolidityEventFinder, eth_client::EthSigningClient};

use super::{
    solidity_types::hello_world::{
        HelloWorldServiceManager::{self, NewTaskCreated, TaskResponded},
        IHelloWorldServiceManager::Task,
    },
    HelloWorldFullClient, HelloWorldSimpleClient,
};

use alloy::{
    primitives::{keccak256, Address, FixedBytes},
    providers::Provider,
    signers::Signer,
    sol_types::SolValue,
};
use anyhow::{ensure, Context, Result};

impl HelloWorldFullClient {
    pub fn into_simple(self) -> HelloWorldSimpleClient {
        HelloWorldSimpleClient::new(self.eth, self.hello_world.hello_world_service_manager)
    }
}

impl HelloWorldSimpleClient {
    pub fn new(eth: EthSigningClient, contract_address: Address) -> Self {
        let contract = HelloWorldServiceManager::new(contract_address, eth.http_provider.clone());
        Self {
            eth,
            contract_address,
            contract,
        }
    }

    pub async fn task_responded_hash(&self, task_index: u32) -> Result<Vec<u8>> {
        let resp = self
            .contract
            .allTaskResponses(self.eth.address(), task_index)
            .call()
            .await
            .context("Failed to query task responses")?
            ._0;

        Ok(resp.to_vec())
    }

    pub async fn create_new_task(&self, task_name: String) -> Result<NewTaskCreated> {
        let hello_world_service_manager =
            HelloWorldServiceManager::new(self.contract_address, self.eth.http_provider.clone());

        let new_task_created: NewTaskCreated = hello_world_service_manager
            .createNewTask(task_name)
            .send()
            .await?
            .get_receipt()
            .await?
            .solidity_event()
            .context("Not found new task creation event")?;
        Ok(new_task_created)
    }

    pub async fn sign_and_submit_task(
        &self,
        task: Task,
        task_index: u32,
    ) -> Result<FixedBytes<32>> {
        tracing::debug!("Signing and responding to task index {}", task_index);

        let signature = self.sign_task_result(&task.name).await?;

        self.submit_task(task, task_index, signature).await
    }

    pub async fn sign_task_result(&self, name: &str) -> Result<Vec<u8>> {
        let message = format!("Hello, {}", name);
        let message_hash = keccak256(message);
        let message_bytes = message_hash.as_slice();
        // TODO: Sign hash or sign message?
        let signature = self.eth.signer.sign_message(message_bytes).await?;
        let operators = vec![self.eth.address()];
        let signatures = vec![signature.as_bytes().to_vec()];

        let reference_block = self.eth.http_provider.get_block_number().await?;

        let signed_task = (operators, signatures, reference_block).abi_encode();

        Ok(signed_task)
    }

    pub async fn submit_task(
        &self,
        task: Task,
        task_index: u32,
        signature: Vec<u8>,
    ) -> Result<FixedBytes<32>> {
        let contract =
            HelloWorldServiceManager::new(self.contract_address, self.eth.http_provider.clone());

        let receipt = contract
            .respondToTask(task, task_index, signature.into())
            .gas(500000)
            .send()
            .await?
            .get_receipt()
            .await?;

        ensure!(receipt.status(), "Failed to submit task");

        let task_responded: TaskResponded = receipt
            .solidity_event()
            .context("Expected TaskResponded event")?;
        tracing::debug!(
            "Responded to a task: {}, by {}",
            task_responded.taskIndex,
            task_responded.operator
        );

        let tx_hash = receipt.transaction_hash;

        Ok(tx_hash)
    }
}
