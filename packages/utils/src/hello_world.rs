use alloy::primitives::{Address, TxHash};
use config::HelloWorldAddresses;
use serde::{Deserialize, Serialize};
use solidity_types::HelloWorldServiceManagerT;
mod avs_operator;

use crate::{eigen_client::CoreAVSAddresses, eth_client::EthSigningClient};

pub mod builder;
pub mod client;
pub mod config;
pub mod solidity_types;

pub struct HelloWorldFullClientBuilder {
    pub eth: EthSigningClient,
    pub core_avs_addrs: Option<CoreAVSAddresses>,
}

impl HelloWorldFullClientBuilder {
    pub fn new(eth: EthSigningClient) -> Self {
        Self {
            eth,
            core_avs_addrs: None,
        }
    }

    pub fn avs_addresses(mut self, addresses: CoreAVSAddresses) -> Self {
        self.core_avs_addrs = Some(addresses);
        self
    }
}

pub struct HelloWorldFullClient {
    pub eth: EthSigningClient,
    pub core: CoreAVSAddresses,
    pub hello_world: HelloWorldAddresses,
}

pub struct HelloWorldSimpleClient {
    pub eth: EthSigningClient,
    pub contract_address: Address,
    pub contract: HelloWorldServiceManagerT,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddTaskRequest {
    pub task_id: String,
    /// Address of the avs
    pub service: Address,
    pub operator: Address,
    pub new_data: TaskData,
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskData {
    pub name: String,
    pub task_index: u32,
    pub task_created_block: u32,
}

/// Returns hash in case there if threshold reached
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddTaskResponse {
    pub hash: Option<TxHash>,
}
