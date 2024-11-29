use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{config::Config, context::AppContext};
use alloy::{
    contract::CallBuilder,
    network::{Ethereum, Network},
    primitives::Address,
    providers::{PendingTransactionError, Provider},
    rpc::types::TransactionRequest,
    sol,
    sol_types::SolValue,
};
use tokio::sync::mpsc;
use tracing::instrument;
use utils::eth_client::EthSigningClient;

sol! {
    #[sol(rpc)]
    contract HelloWorldServiceManager {
        constructor(address) {} // The `deploy` method will also include any constructor arguments.

        #[derive(Debug)]
        struct Task {
            string name;
            uint32 taskCreatedBlock;
        }

        #[derive(Debug)]
        function respondToTask(
            Task calldata task,
            uint32 referenceTaskIndex,
            bytes memory signature
        ) external;
    }
}

#[derive(Clone)]
pub struct EthSubmission {}

impl EthSubmission {
    pub async fn submit(
        &self,
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
        let contract = HelloWorldServiceManager::new(hello_world_address, client.provider);
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
