use std::{fmt, ops::Deref};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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

impl Trigger {
    pub fn queue(task_queue_addr: &str, poll_interval: u32) -> Self {
        Trigger::Queue {
            task_queue_addr: task_queue_addr.to_string(),
            poll_interval,
        }
    }
}

// TODO: custom Deserialize that enforces validation rules
/// ID is meant to identify a component or a service (I don't think we need to enforce the distinction there, do we?)
/// It is a string, but with some strict validation rules. It must be lowecase alphanumeric: `[a-z0-9-_]{3,32}`
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct ID(String);

impl ID {
    pub fn new(id: &str) -> Result<Self, IDError> {
        if id.len() < 3 || id.len() > 32 {
            return Err(IDError::LengthError);
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_numeric() || c == '_' || c == '-')
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

impl Deref for ID {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for ID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for ID {
    type Error = IDError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        ID::new(s)
    }
}

// makes it easier to use in T: TryInto
impl TryFrom<&ID> for ID {
    type Error = IDError;

    fn try_from(id: &ID) -> Result<Self, Self::Error> {
        Ok(id.clone())
    }
}

#[derive(Error, Debug, PartialEq, Clone)]
pub enum IDError {
    #[error("ID must be between 3 and 32 characters")]
    LengthError,
    #[error("ID must be lowercase alphanumeric")]
    CharError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids() {
        ID::new("foobar").unwrap();
        ID::new("foot123").unwrap();
        ID::new("123foot").unwrap();
        ID::new("two_words").unwrap();
        ID::new("kebab-case").unwrap();
        ID::new("pretty-1234321-long").unwrap();
        // 32 chars
        ID::new("12345678901234567890123456789012").unwrap();
    }

    #[test]
    fn invalid_ids() {
        // test length
        let err = ID::new("fo").unwrap_err();
        assert_eq!(err, IDError::LengthError);
        let err = ID::new("123456789012345678901234567890123").unwrap_err();
        assert_eq!(err, IDError::LengthError);

        // test chars
        let err = ID::new("with space").unwrap_err();
        assert_eq!(err, IDError::CharError);
        ID::new("UPPER_SPACE").unwrap_err();
        ID::new("Capitalized").unwrap_err();
        ID::new("../../etc/passwd").unwrap_err();
        ID::new("c:\\\\badfile").unwrap_err();
    }

    #[test]
    fn proper_representation() {
        let name = "fly2you";
        let id = ID::new(name).unwrap();
        // same string rep
        assert_eq!(id.to_string(), name.to_string());
        // can be used AsRef
        assert_eq!(name, id.as_ref());
        // deref working (call method from &str on ID)
        assert_eq!(name.as_bytes(), id.as_bytes())
    }
}
