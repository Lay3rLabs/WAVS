use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    // TODO: add this variant later, not for now
    // #[serde(rename_all = "camelCase")]
    // Cron { schedule: String },
    #[serde(rename_all = "camelCase")]
    Queue {
        // FIXME: add some chain name. right now all triggers are on one chain
        task_queue_addr: String,
        /// Frequency in seconds to poll the task queue (doubt this is over 3600 ever, but who knows)
        poll_interval: u32,
    },
}

// TODO: custom Deserialize that enforces validation rules
/// ID is meant to identify a component or a service (I don't think we need to enforce the distinction there, do we?)
/// It is a string, but with some strict validation rules. It must be lowecase alphanumeric: `[a-z0-9-_]{3,32}`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct ID(String);

impl ID {
    pub fn new(id: &str) -> Result<Self, IDError> {
        if id.len() < 3 || id.len() > 32 {
            return Err(IDError::LengthError);
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() && c.is_alphanumeric())
        {
            return Err(IDError::CharError);
        }
        Ok(Self(id.to_string()))
    }
}

impl AsRef<str> for ID {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Error, Debug)]
pub enum IDError {
    #[error("ID must be between 3 and 32 characters")]
    LengthError,
    #[error("ID must be lowercase alphanumeric")]
    CharError,
}
