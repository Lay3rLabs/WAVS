use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use alloy::primitives::Address;

use crate::config::Config;

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub aggregator_state: Arc<RwLock<HashMap<String, Task>>>,
}

#[derive(Clone)]
pub struct Task {
    pub signatures: HashMap<Address, Vec<u8>>,
    pub operators: Vec<Address>,
    pub avl: Address,
    pub reference_block: u64,
    pub function: alloy::json_abi::Function,
    /// Function input without a signature
    pub input: Vec<u8>,
    pub erc1271: Address,
}

impl HttpState {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        Ok(Self {
            config,
            aggregator_state: Default::default(),
        })
    }
}
