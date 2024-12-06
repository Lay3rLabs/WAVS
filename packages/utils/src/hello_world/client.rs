//! Hello world client implementations
//!

use crate::{
    alloy_helpers::SolidityEventFinder,
    eth_client::{AddMessageRequest, EthSigningClient},
};

use super::{
    solidity_types::hello_world::{
        HelloWorldServiceManager::{self, NewTaskCreated, TaskResponded},
        IHelloWorldServiceManager::Task,
    },
    HelloWorldFullClient, HelloWorldSimpleClient,
};

use alloy::{
    dyn_abi::DynSolValue,
    json_abi::Function,
    primitives::{eip191_hash_message, keccak256, Address, FixedBytes, U256},
    providers::Provider,
    signers::SignerSync,
    sol_types::{SolCall, SolValue},
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
    ) -> Result<AddMessageRequest> {
        tracing::debug!("Signing and responding to task index {}", task_index);

        let signature = self.sign_task_result(&task.name).await?;

        let contract =
            HelloWorldServiceManager::new(self.contract_address, self.eth.http_provider.clone());

        let function = HelloWorldServiceManager::abi::functions()
            .get("respondToTask")
            .unwrap()
            .first()
            .unwrap()
            .clone();
        let task_name = task.name.clone();
        let call = HelloWorldServiceManager::respondToTaskCall {
            task,
            referenceTaskIndex: task_index,
            // Filled by aggregator
            signature: Default::default(),
        };
        let function_input = call.abi_encode();

        Ok(AddMessageRequest {
            operators: vec![self.eth.address()],
            signature,
            task_name,
            avl: *contract.address(),
            function,
            function_input,
        })
    }

    pub async fn sign_task_result(&self, name: &str) -> Result<Vec<u8>> {
        let message = format!("Hello, {}", name);
        let message_hash = eip191_hash_message(keccak256(message.abi_encode_packed()));
        // TODO: Sign hash or sign message?
        let operators: Vec<DynSolValue> = vec![DynSolValue::Address(self.eth.address())];
        let signature: Vec<DynSolValue> = vec![DynSolValue::Bytes(
            self.eth.signer.sign_hash_sync(&message_hash)?.into(),
        )];

        let current_block = U256::from(self.eth.http_provider.get_block_number().await?);

        let signed_task = DynSolValue::Tuple(vec![
            DynSolValue::Array(operators),
            DynSolValue::Array(signature),
            DynSolValue::Uint(current_block, 32),
        ])
        .abi_encode_params();

        Ok(signed_task)
    }

    pub async fn submit_task(
        &self,
        task: Task,
        task_index: u32,
        signature: Vec<u8>,
        aggregator_endpoint: String,
    ) -> Result<AddMessageRequest> {
        todo!()
    }
}
