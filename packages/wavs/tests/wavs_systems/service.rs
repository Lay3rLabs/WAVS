use std::sync::LazyLock;

use crate::{
    apis::{dispatcher::Permissions, Trigger},
    http::types::app::{App, Status},
    Digest,
};

use super::chain::MOCK_TASK_QUEUE_ADDRESS;

pub struct MockServiceBuilder {
    pub name: String,
    pub status: Option<Option<Status>>,
    pub trigger: Option<Trigger>,
    pub testable: Option<Option<bool>>,
}

impl MockServiceBuilder {
    pub fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            status: None,
            trigger: None,
            testable: None,
        }
    }

    pub fn with_status(mut self, status: Option<Status>) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_trigger(mut self, trigger: Trigger) -> Self {
        self.trigger = Some(trigger);
        self
    }

    pub fn with_testable(mut self, testable: Option<bool>) -> Self {
        self.testable = Some(testable);
        self
    }

    pub fn build(self) -> App {
        static DIGEST: LazyLock<Digest> = LazyLock::new(|| Digest::new([0; 32]));

        App {
            trigger: self.trigger.unwrap_or(Trigger::Queue {
                task_queue_addr: MOCK_TASK_QUEUE_ADDRESS.to_string(),
                poll_interval: 1000,
            }),
            name: self.name,
            status: self.status.unwrap_or_default(),
            digest: DIGEST.clone().into(),
            permissions: Permissions::default(),
            testable: self.testable.unwrap_or_default(),
        }
    }
}
