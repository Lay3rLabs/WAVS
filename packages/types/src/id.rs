use serde::{Deserialize, Deserializer, Serialize};
use std::{ops::Deref, str::FromStr};
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum IDError {
    #[error("ID must be between 3 and 36 characters")]
    LengthError,
    #[error("ID must be lowercase alphanumeric")]
    CharError,
}

/// Macro for generating new ID like types
macro_rules! new_id_type {
    ($type_name:ident) => {
        /// It is a string, but with some strict validation rules. It must be lowercase alphanumeric: `[a-z0-9-_]{3,36}`
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
        pub struct $type_name(String);

        impl $type_name {
            // take Into<String> instead of ToString so we benefit from zero-cost conversions for common cases
            // String -> String is a no-op
            // &str -> String is via std lib magic (internal transmute, ultimately)
            pub fn new(id: impl Into<String>) -> Result<Self, IDError> {
                let id = id.into();

                if id.len() < 3 || id.len() > 36 {
                    return Err(IDError::LengthError);
                }
                if !id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_numeric() || c == '_' || c == '-')
                {
                    return Err(IDError::CharError);
                }
                Ok(Self(id))
            }
        }

        impl<'de> Deserialize<'de> for $type_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                $type_name::new(s).map_err(serde::de::Error::custom)
            }
        }

        impl AsRef<str> for $type_name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl Deref for $type_name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::fmt::Display for $type_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl TryFrom<&str> for $type_name {
            type Error = IDError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                $type_name::new(s)
            }
        }

        // makes it easier to use in T: TryInto
        impl TryFrom<&$type_name> for $type_name {
            type Error = IDError;

            fn try_from(id: &Self) -> Result<Self, Self::Error> {
                Ok(id.clone())
            }
        }
    };
}

new_id_type!(ServiceID);
new_id_type!(WorkflowID);
// Distinct from a ChainConfig's ChainID - this is the *name* used within WAVS
// It's allowed for multiple chains to have the same ChainID, but ChainName is unique
new_id_type!(ChainName);

// Define FromStr for ServiceID to enable parsing from command line strings
impl FromStr for ServiceID {
    type Err = IDError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ServiceID::new(s.to_string())
    }
}

impl FromStr for WorkflowID {
    type Err = IDError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkflowID::new(s.to_string())
    }
}

impl FromStr for ChainName {
    type Err = IDError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ChainName::new(s.to_string())
    }
}

impl Default for WorkflowID {
    fn default() -> Self {
        WorkflowID::new("default").unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids() {
        ServiceID::new("foobar").unwrap();
        ServiceID::new("foot123").unwrap();
        ServiceID::new("123foot").unwrap();
        ServiceID::new("two_words").unwrap();
        ServiceID::new("kebab-case").unwrap();
        ServiceID::new("pretty-1234321-long").unwrap();
        // 32 chars
        ServiceID::new("12345678901234567890123456789012").unwrap();
    }

    #[test]
    fn invalid_ids() {
        // test length
        let err = ServiceID::new("fo").unwrap_err();
        assert_eq!(err, IDError::LengthError);
        let err = ServiceID::new("1234567890123456789012345678901234567").unwrap_err();
        assert_eq!(err, IDError::LengthError);

        // test chars
        let err = ServiceID::new("with space").unwrap_err();
        assert_eq!(err, IDError::CharError);
        ServiceID::new("UPPER_SPACE").unwrap_err();
        ServiceID::new("Capitalized").unwrap_err();
        ServiceID::new("../../etc/passwd").unwrap_err();
        ServiceID::new("c:\\\\badfile").unwrap_err();
    }

    #[test]
    fn invalid_id_deserialize() {
        // baseline, make sure we can deserialize properly
        let id_str = "foo";
        let id_obj: ServiceID = serde_json::from_str(&format!("\"{id_str}\"")).unwrap();
        assert_eq!(id_obj.to_string(), id_str);

        // now do a bad id
        let id_str = "THIS/IS/BAD";
        serde_json::from_str::<ServiceID>(&format!("\"{id_str}\"")).unwrap_err();
    }

    #[test]
    fn proper_representation() {
        let name = "fly2you";
        let id = ServiceID::new(name).unwrap();
        // same string rep
        assert_eq!(id.to_string(), name.to_string());
        // can be used AsRef
        assert_eq!(name, id.as_ref());
        // deref working (call method from &str on ID)
        assert_eq!(name.as_bytes(), id.as_bytes())
    }
}
