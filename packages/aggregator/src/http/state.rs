use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use alloy::primitives::Address;

use crate::config::Config;

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub aggregator_state: Arc<RwLock<HashMap<(String, Address), Vec<Task>>>>,
}

impl HttpState {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        Ok(Self {
            config,
            aggregator_state: Default::default(),
        })
    }

    pub fn load(&self, key: &(String, Address)) -> Vec<Task> {
        self.aggregator_state
            .read()
            .unwrap()
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub fn save(&self, key: (String, Address), value: Vec<Task>) {
        self.aggregator_state.write().unwrap().insert(key, value);
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    pub operator: Address,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}

impl Task {
    pub fn new(operator: Address, data: Vec<u8>, signature: Vec<u8>) -> Self {
        Self {
            operator,
            data,
            signature,
        }
    }
}
