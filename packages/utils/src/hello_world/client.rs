//! Hello world client implementations
//!

use crate::{
    alloy_helpers::SolidityEventFinder,
    eth_client::EthSigningClient,
    hello_world::{
        solidity_types::hello_world::HelloWorldServiceManager::TaskResponded, AddTaskRequest,
        TaskData,
    },
};

use super::{
    solidity_types::hello_world::{
        HelloWorldServiceManager::{self, NewTaskCreated},
        IHelloWorldServiceManager::Task,
    },
    HelloWorldFullClient, HelloWorldSimpleClient,
};

use alloy::{
    dyn_abi::DynSolValue,
    primitives::{keccak256, Address, FixedBytes, U256},
    providers::Provider,
    signers::SignerSync,
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
        let mut resp = self
            .contract
            .allTaskResponses(self.eth.address(), task_index)
            .call()
            .await
            .context("Failed to query task responses")?
            ._0;
        // If response is empty, check for self multicall
        if resp.is_empty() {
            resp = self
                .contract
                .allTaskResponses(self.contract_address, task_index)
                .call()
                .await
                .context("Failed to query task responses")?
                ._0;
        }

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

        let signature = self.sign_task(&task.name).await?;

        self.submit_task(task, task_index, signature).await
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

    pub async fn task_request(&self, task: Task, task_index: u32) -> Result<AddTaskRequest> {
        tracing::debug!("Signing and responding to task index {}", task_index);

        let signature = self.sign_task(&task.name).await?;

        let contract =
            HelloWorldServiceManager::new(self.contract_address, self.eth.http_provider.clone());

        let new_data = TaskData {
            name: task.name,
            task_index,
            task_created_block: task.taskCreatedBlock,
        };

        let operator = self.eth.address();
        let service = *contract.address();
        let task_id = "respond_to_task".to_owned();
        Ok(AddTaskRequest {
            service,
            task_id,
            operator,
            new_data,
            signature,
        })
    }

    pub async fn sign_task(&self, name: &str) -> Result<Vec<u8>> {
        let message = format!("Hello, {name}");
        let operator_signature = DynSolValue::Bytes(
            self.eth
                .signer
                .sign_message_sync(keccak256(message.abi_encode_packed()).as_slice())?
                .into(),
        );

        let operator = DynSolValue::Address(self.eth.address());
        let reference_block = self.eth.http_provider.get_block_number().await?;
        let signature = DynSolValue::Tuple(vec![
            DynSolValue::Array(vec![operator]),
            DynSolValue::Array(vec![operator_signature]),
            DynSolValue::Uint(U256::from(reference_block), 32),
        ])
        .abi_encode_params();

        Ok(signature)
    }
}
