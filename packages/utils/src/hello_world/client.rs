//! Hello world client implementations
//!

use crate::alloy_helpers::SolidityEventFinder;

use super::{
    solidity_types::hello_world::HelloWorldServiceManager::{self, NewTaskCreated},
    HelloWorldClient,
};

use alloy::{primitives::keccak256, providers::Provider, signers::Signer, sol_types::SolValue};
use anyhow::{Context, Result};

impl HelloWorldClient {
    pub async fn create_new_task(&self, task_name: String) -> Result<NewTaskCreated> {
        let hello_world_service_manager = HelloWorldServiceManager::new(
            self.hello_world.hello_world_service_manager,
            self.eth.http_provider.clone(),
        );

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

    pub async fn sign_and_respond_to_task(&self, new_task_event: NewTaskCreated) -> Result<()> {
        let hello_world_service_manager = HelloWorldServiceManager::new(
            self.hello_world.hello_world_service_manager,
            self.eth.http_provider.clone(),
        );

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

        let reference_block = self.eth.ws_provider.get_block_number().await?;

        let signed_task = (operators, signatures, reference_block).abi_encode();

        let response_hash = hello_world_service_manager
            .respondToTask(
                new_task_event.task,
                new_task_event.taskIndex,
                signed_task.into(),
            )
            .gas(500000)
            .send()
            .await?
            .get_receipt()
            .await?
            .transaction_hash;
        tracing::debug!("Responded to task with tx hash {}", response_hash);

        Ok(())
    }
}
