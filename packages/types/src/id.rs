use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest as Sha2Digest, Sha256};
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
macro_rules! new_string_id_type {
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

macro_rules! new_hash_id_type {
    ($type_name:ident, true) => {
        new_hash_id_type!(@base $type_name);
        new_hash_id_type!(@intoany $type_name);
    };

    ($type_name:ident, false) => {
        new_hash_id_type!(@base $type_name);
    };

    // use "@base" as a way of marking the base implementation
    (@base $type_name:ident) => {
        /// It is a string, but hex-encoded 32-byte hash
        #[derive(
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            ToSchema,
            bincode::Decode,
            bincode::Encode,
        )]
        pub struct $type_name([u8; 32]);

        impl $type_name {
            pub fn new(bytes: impl AsRef<[u8]>) -> Self {
                let mut digest = [0u8; 32];
                let mut hasher = Sha256::new();
                hasher.update(bytes);
                hasher.finalize_into((&mut digest).into());
                $type_name(digest)
            }

            pub fn inner(&self) -> [u8;32] {
                self.0
            }

        }

        impl From<[u8; 32]> for $type_name {
            fn from(value: [u8; 32]) -> Self {
                $type_name(value)
            }
        }

        impl AsRef<[u8]> for $type_name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        impl std::fmt::Display for $type_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}",const_hex::encode(self.0.as_slice()))
            }
        }

        impl std::fmt::Debug for $type_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self)
            }
        }

        impl FromStr for $type_name {
            type Err = const_hex::FromHexError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let mut bytes = [0u8; 32];
                const_hex::decode_to_slice(s, &mut bytes)?;
                Ok($type_name(bytes))
            }
        }

        impl Serialize for $type_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $type_name {
            fn deserialize<D>(deserializer: D) -> Result<$type_name, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct StrVisitor;

                impl<'de> serde::de::Visitor<'de> for StrVisitor {
                    type Value = $type_name;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("expected hex-encoded string")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        $type_name::from_str(value).map_err(serde::de::Error::custom)
                    }
                }

                deserializer.deserialize_str(StrVisitor)
            }
        }
    };

    // use "@intoany" as a way of marking the Into<AnyDigest> implementation
    // we intentionally do NOT implement From<AnyDigest> for $type_name
    // because we want to avoid accidental conversions
    // in other words - it's fine to erase the type, it's not fine to assume something specific from the erased type
    (@intoany $type_name:ident) => {
        impl From<$type_name> for AnyDigest {
            fn from(digest: $type_name) -> Self {
                AnyDigest(digest.inner())
            }
        }
    };
}

new_string_id_type!(ServiceID);
new_string_id_type!(WorkflowID);
// Distinct from a ChainConfig's ChainID - this is the *name* used within WAVS
// It's allowed for multiple chains to have the same ChainID, but ChainName is unique
new_string_id_type!(ChainName);

// This is just used as a general purpose digest type
new_hash_id_type!(AnyDigest, false);
// Digest of the whole Service definition
new_hash_id_type!(ServiceDigest, true);
// Digest of the component source (e.g. wasm bytecode)
new_hash_id_type!(ComponentDigest, true);
// Digest of any data, used for generic storage

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
