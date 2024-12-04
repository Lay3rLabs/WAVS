//! Hello world client implementations
//!

use crate::alloy_helpers::SolidityEventFinder;

use super::{
    solidity_types::hello_world::HelloWorldServiceManager::{self, NewTaskCreated},
    HelloWorldClient,
};

use anyhow::{Context, Result};

impl HelloWorldClient {
    pub async fn create_new_task(&self, task_name: String) -> Result<NewTaskCreated> {
        let hello_world_service_manager = HelloWorldServiceManager::new(
            self.hello_world
                .addresses
                .hello_world_service_manager
                .clone(),
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
}
