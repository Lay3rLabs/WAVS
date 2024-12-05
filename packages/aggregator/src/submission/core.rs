use alloy::{
    primitives::Address,
    providers::{PendingTransactionError, Provider},
    sol_types::SolValue,
};
use utils::eth_client::EthSigningClient;

use crate::alloy_generated_types::HelloWorldServiceManager;

#[derive(Clone)]
pub struct EthSubmission {}

impl EthSubmission {
    pub async fn submit(
        task_index: u32,
        task_name: String,
        task_created_block: u32,
        client: EthSigningClient,
        operators: Vec<Address>,
        signatures: Vec<Vec<u8>>,
        reference_block: u32,
        hello_world_address: Address,
    ) -> Result<(), SubmissionError> {
        let signed_task = (operators, signatures, reference_block).abi_encode();
        let contract = HelloWorldServiceManager::new(hello_world_address, client.http_provider);
        let respond_to_task = contract.respondToTask(
            HelloWorldServiceManager::Task {
                name: task_name,
                taskCreatedBlock: task_created_block,
            },
            task_index,
            signed_task.into(),
        );
        respond_to_task.call().await?;

        // Call without error, let's send
        let pending_tx = respond_to_task.send().await?;
        let tx_hash = pending_tx.watch().await?;
        eprintln!("Hello world txhash:{}", tx_hash.to_string());
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SubmissionError {
    #[error(transparent)]
    AlloyContractError(#[from] alloy::contract::Error),

    #[error(transparent)]
    PendingTransactionError(#[from] PendingTransactionError),
}
