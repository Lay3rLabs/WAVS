use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest as Sha2Digest, Sha256};
use std::str::FromStr;
use utoipa::ToSchema;

#[cfg(feature = "ts-bindings")]
use ts_rs::TS;

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
        #[cfg_attr(feature = "ts-bindings", derive(TS))]
        #[cfg_attr(feature = "ts-bindings", ts(export, type = "string"))]
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
            pub fn hash(bytes: impl AsRef<[u8]>) -> Self {
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

// This is just used as a general purpose digest type
new_hash_id_type!(AnyDigest, false);
// Digest of the whole Service definition
new_hash_id_type!(ServiceDigest, true);
// Digest of the component source (e.g. wasm bytecode)
new_hash_id_type!(ComponentDigest, true);

// ServiceId is a unique identifier for a service
// it's a hash of the ServiceManager definition (chain and address)
new_hash_id_type!(ServiceId, true);
