use std::time::Duration;

use lavs_apis::id::TaskId;
use utils::{
    eth_client::EthSigningClient,
    hello_world::{
        solidity_types::hello_world::HelloWorldServiceManager::NewTaskCreated,
        HelloWorldSimpleClient,
    },
};

pub async fn run_hello_world_task(
    eth_signing_client: EthSigningClient,
    wavs: bool,
    contract_address: alloy::primitives::Address,
    name: String,
) -> String {
    let client = HelloWorldSimpleClient::new(eth_signing_client, contract_address);

    let NewTaskCreated { task, taskIndex } = client.create_new_task(name).await.unwrap();

    println!("Task submitted with id: {}", TaskId::new(taskIndex as u64));

    if !wavs {
        tracing::info!("Submitting the task result directly");

        client.sign_and_submit_task(task, taskIndex).await.unwrap();
    }

    tracing::info!("Waiting for the chain to see the result");

    tokio::time::timeout(Duration::from_secs(10), async move {
        loop {
            let task_response_hash = client.task_responded_hash(taskIndex).await.unwrap();

            if !task_response_hash.is_empty() {
                return hex::encode(task_response_hash);
            } else {
                tracing::info!(
                    "Waiting for task response by {} on {} for index {}...",
                    client.eth.address(),
                    client.contract_address,
                    taskIndex
                );
            }
            // still open, waiting...
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap()
}
