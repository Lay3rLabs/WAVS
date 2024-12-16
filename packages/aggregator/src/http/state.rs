use std::{collections::HashMap, sync::Arc};

use alloy::primitives::Address;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use utils::{
    hello_world::TaskData,
    storage::db::{DBError, RedbStorage, Table, JSON},
};

use crate::config::Config;

// Task Id -> Tasks
pub type TasksMap = HashMap<String, Vec<Task>>;

// Note: If service exists in db it's considered registered
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
        Ok(Self { config, storage })
    }

    pub fn load_tasks(&self, service: Address) -> anyhow::Result<TasksMap> {
        match self.storage.get(TASKS, &service.to_string())? {
            Some(tasks) => Ok(tasks.value()),
            None => Err(anyhow::anyhow!("Task not registered")),
        }
    }

    pub fn save_tasks(&self, service: Address, tasks: TasksMap) -> Result<(), DBError> {
        self.storage.set(TASKS, &service.to_string(), &tasks)
    }

    pub fn register_service(&self, service: Address) -> anyhow::Result<()> {
        let service = service.to_string();
        match self.storage.get(TASKS, &service)? {
            Some(_) => {
                bail!("Service is already registered");
            }
            None => {
                self.storage.set(TASKS, &service, &HashMap::default())?;
            }
        }
        Ok(())
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
