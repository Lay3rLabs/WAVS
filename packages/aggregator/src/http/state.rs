use std::{collections::HashMap, sync::Arc};

use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use utils::{
    hello_world::TaskData,
    storage::db::{DBError, RedbStorage, Table, JSON},
};

use crate::config::Config;

// Service address -> Tasks
pub type TasksMap = HashMap<Address, Vec<Task>>;

const TASKS: Table<&str, JSON<TasksMap>> = Table::new("tasks");

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub storage: Arc<RedbStorage>,
}

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let storage = Arc::new(RedbStorage::new(config.data.join("db"))?);
        Ok(Self {
            config,
            storage: storage,
        })
    }

    pub fn load_tasks(&self, task_id: &str) -> anyhow::Result<TasksMap> {
        match self.storage.get(TASKS, task_id)? {
            Some(tasks) => Ok(tasks.value()),
            None => Err(anyhow::anyhow!("Task not registered")),
        }
    }

    pub fn save_tasks(&self, task_id: &str, tasks: TasksMap) -> Result<(), DBError> {
        self.storage.set(TASKS, task_id, &tasks)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub operator: Address,
    pub data: TaskData,
    pub signature: Vec<u8>,
}

impl Task {
    pub fn new(operator: Address, data: TaskData, signature: Vec<u8>) -> Self {
        Self {
            operator,
            data,
            signature,
        }
    }
}
