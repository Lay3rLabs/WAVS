//! Hello world client implementations
//!

use crate::{
    alloy_helpers::SolidityEventFinder,
    eth_client::{AddTaskRequest, EthSigningClient, OperatorSignature},
};

use super::{
    solidity_types::hello_world::{
        HelloWorldServiceManager::{self, NewTaskCreated},
        IHelloWorldServiceManager::Task,
    },
    HelloWorldFullClient, HelloWorldSimpleClient,
};

use alloy::{
    primitives::{eip191_hash_message, keccak256, Address},
    providers::Provider,
    signers::SignerSync,
    sol_types::{SolCall, SolValue},
};
use anyhow::{Context, Result};

impl HelloWorldFullClient {
    pub fn into_simple(self) -> HelloWorldSimpleClient {
        HelloWorldSimpleClient::new(
            self.eth,
            self.hello_world.hello_world_service_manager,
            self.hello_world.stake_registry,
        )
    }
}

impl HelloWorldSimpleClient {
    pub fn new(eth: EthSigningClient, contract_address: Address, erc1271: Address) -> Self {
        let contract = HelloWorldServiceManager::new(contract_address, eth.http_provider.clone());
        Self {
            eth,
            contract_address,
            contract,
            erc1271,
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

    pub async fn task_request(&self, task: Task, task_index: u32) -> Result<AddTaskRequest> {
        tracing::debug!("Signing and responding to task index {}", task_index);

        let signature = self.sign_task(&task.name).await?;

        let contract =
            HelloWorldServiceManager::new(self.contract_address, self.eth.http_provider.clone());

        let function = HelloWorldServiceManager::abi::functions()
            .get("respondToTask")
            .unwrap()
            .first()
            .unwrap()
            .clone();
        let task_name = format!("Hello, {}", task.name);
        let call = HelloWorldServiceManager::respondToTaskCall {
            task,
            referenceTaskIndex: task_index,
            // Filled by aggregator
            signature: Default::default(),
        };
        let mut function_input = Vec::with_capacity(call.abi_encoded_size());
        call.abi_encode_raw(&mut function_input);

        let operator = self.eth.address();
        let reference_block = self.eth.http_provider.get_block_number().await? - 1;
        Ok(AddTaskRequest {
            task_name,
            avl: *contract.address(),
            function,
            input: function_input,
            reference_block,
            // TODO: fill other operators
            operators: vec![operator],
            signature: OperatorSignature {
                address: operator,
                signature,
            },
            erc1271: self.erc1271,
        })
    }

    pub async fn sign_task(&self, name: &str) -> Result<Vec<u8>> {
        let message = format!("Hello, {name}");
        let message_hash = eip191_hash_message(keccak256(message.abi_encode_packed()));
        // TODO: Sign hash or sign message?
        let signature: Vec<u8> = self.eth.signer.sign_hash_sync(&message_hash)?.into();

        Ok(signature)
    }
}
