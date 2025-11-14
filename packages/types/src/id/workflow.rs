use serde::{Deserialize, Deserializer, Serialize};
use std::{ops::Deref, str::FromStr};
use thiserror::Error;
#[cfg(feature = "ts-bindings")]
use ts_rs::TS;
use utoipa::ToSchema;

/// It is a string, but with some strict validation rules. It must be lowercase alphanumeric: `[a-z0-9-_]{3,36}`
#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(type = "string"))]
#[derive(
    Serialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ToSchema,
    bincode::Decode,
    bincode::Encode,
)]
#[serde(transparent)]
pub struct WorkflowId(String);

impl WorkflowId {
    /// Validates without taking ownership - good for checking
    pub fn validate(id: impl AsRef<str>) -> Result<(), WorkflowIdError> {
        let id = id.as_ref();

        if id.len() < 3 || id.len() > 36 {
            Err(WorkflowIdError::LengthError)
        } else if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_numeric() || c == '_' || c == '-')
        {
            Err(WorkflowIdError::CharError)
        } else {
            Ok(())
        }
    }
    // take Into<String> instead of ToString so we benefit from zero-cost conversions for common cases
    // String -> String is a no-op
    // &str -> String is via std lib magic (internal transmute, ultimately)
    pub fn new(id: impl Into<String>) -> Result<Self, WorkflowIdError> {
        let id = id.into();

        Self::validate(&id)?;

        Ok(Self(id))
    }
}

impl<'de> Deserialize<'de> for WorkflowId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl AsRef<str> for WorkflowId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for WorkflowId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for WorkflowId {
    type Error = WorkflowIdError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl FromStr for WorkflowId {
    type Err = WorkflowIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl Default for WorkflowId {
    fn default() -> Self {
        WorkflowId::new("default").unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids() {
        WorkflowId::new("foobar").unwrap();
        WorkflowId::new("foot123").unwrap();
        WorkflowId::new("123foot").unwrap();
        WorkflowId::new("two_words").unwrap();
        WorkflowId::new("kebab-case").unwrap();
        WorkflowId::new("pretty-1234321-long").unwrap();
        // 32 chars
        WorkflowId::new("12345678901234567890123456789012").unwrap();
    }

    #[test]
    fn invalid_ids() {
        // test length
        let err = WorkflowId::new("fo").unwrap_err();
        assert_eq!(err, WorkflowIdError::LengthError);
        let err = WorkflowId::new("1234567890123456789012345678901234567").unwrap_err();
        assert_eq!(err, WorkflowIdError::LengthError);

        // test chars
        let err = WorkflowId::new("with space").unwrap_err();
        assert_eq!(err, WorkflowIdError::CharError);
        WorkflowId::new("UPPER_SPACE").unwrap_err();
        WorkflowId::new("Capitalized").unwrap_err();
        WorkflowId::new("../../etc/passwd").unwrap_err();
        WorkflowId::new("c:\\\\badfile").unwrap_err();
    }

    #[test]
    fn invalid_id_deserialize() {
        // baseline, make sure we can deserialize properly
        let id_str = "foo";
        let id_obj: WorkflowId = serde_json::from_str(&format!("\"{id_str}\"")).unwrap();
        assert_eq!(id_obj.to_string(), id_str);

        // now do a bad id
        let id_str = "THIS/IS/BAD";
        serde_json::from_str::<WorkflowId>(&format!("\"{id_str}\"")).unwrap_err();
    }

    #[test]
    fn proper_representation() {
        let name = "fly2you";
        let id = WorkflowId::new(name).unwrap();
        // same string rep
        assert_eq!(id.to_string(), name.to_string());
        // can be used AsRef
        assert_eq!(name, id.as_ref());
        // deref working (call method from &str on ID)
        assert_eq!(name.as_bytes(), id.as_bytes())
    }
}

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum WorkflowIdError {
    #[error("ID must be between 3 and 36 characters")]
    LengthError,
    #[error("ID must be lowercase alphanumeric")]
    CharError,
}
