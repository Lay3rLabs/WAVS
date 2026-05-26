//! Solana / SVM primitive types shared across the WAVS type surface.
//!
//! This module intentionally avoids depending on `solana-sdk` for the v1
//! trigger-only slice — a 32-byte base58-encoded pubkey wrapper and a
//! commitment-level enum are all that the trigger declaration + delivered
//! trigger data need. When slice 2 wires up the actual `solana-client`
//! subscription stream, we may revisit and replace [`SolanaAddress`] with a
//! `solana_sdk::Pubkey` newtype.

use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use utoipa::ToSchema;

#[cfg(feature = "ts-bindings")]
use ts_rs::TS;

/// A 32-byte Solana pubkey (program id, account address, etc.) serialized as
/// a base58 string.
///
/// Solana on-chain addresses are 32 bytes of arbitrary data (the ed25519
/// public key for keypair accounts, a derived hash for PDAs, etc.). The
/// canonical wire format is base58, which is what RPC endpoints, explorers
/// and most tooling speak.
#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export, type = "string"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ToSchema)]
#[schema(value_type = String)]
pub struct SolanaAddress([u8; 32]);

impl SolanaAddress {
    /// Length of a Solana address in bytes.
    pub const LEN: usize = 32;

    /// Construct from raw bytes.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the raw 32-byte representation.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Consume the address and return the raw 32-byte representation.
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Parse a base58-encoded address.
    pub fn from_base58(s: &str) -> Result<Self, SolanaAddressError> {
        let bytes = bs58::decode(s)
            .into_vec()
            .map_err(|e| SolanaAddressError::Base58(e.to_string()))?;
        if bytes.len() != Self::LEN {
            return Err(SolanaAddressError::WrongLength(bytes.len()));
        }
        let mut arr = [0u8; Self::LEN];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Base58-encode the address.
    pub fn to_base58(&self) -> String {
        bs58::encode(self.0).into_string()
    }
}

impl std::fmt::Debug for SolanaAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SolanaAddress({})", self.to_base58())
    }
}

impl std::fmt::Display for SolanaAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_base58())
    }
}

impl FromStr for SolanaAddress {
    type Err = SolanaAddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_base58(s)
    }
}

impl TryFrom<&str> for SolanaAddress {
    type Error = SolanaAddressError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<[u8; 32]> for SolanaAddress {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<SolanaAddress> for [u8; 32] {
    fn from(addr: SolanaAddress) -> Self {
        addr.0
    }
}

impl Serialize for SolanaAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_base58())
    }
}

impl<'de> Deserialize<'de> for SolanaAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_base58(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SolanaAddressError {
    #[error("invalid base58 encoding: {0}")]
    Base58(String),
    #[error("solana address must be 32 bytes, got {0}")]
    WrongLength(usize),
}

/// Commitment level for Solana RPC subscriptions.
///
/// `processed` — the bank has processed the slot but it may still be skipped.
/// `confirmed` — the slot has been voted on by a supermajority of the cluster.
/// `finalized` — the slot is rooted; reorgs above this level are not expected
/// during normal cluster operation.
///
/// Default is `confirmed`, matching the trigger-side recommendation in the
/// SVM design doc.
#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SolanaCommitment {
    Processed,
    #[default]
    Confirmed,
    Finalized,
}

impl std::fmt::Display for SolanaCommitment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolanaCommitment::Processed => f.write_str("processed"),
            SolanaCommitment::Confirmed => f.write_str("confirmed"),
            SolanaCommitment::Finalized => f.write_str("finalized"),
        }
    }
}

impl FromStr for SolanaCommitment {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "processed" => Ok(SolanaCommitment::Processed),
            "confirmed" => Ok(SolanaCommitment::Confirmed),
            "finalized" => Ok(SolanaCommitment::Finalized),
            other => anyhow::bail!(
                "invalid commitment '{}'. Must be one of: processed, confirmed, finalized",
                other
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// System Program — the canonical, well-known address used in tests.
    const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

    #[test]
    fn address_roundtrip_base58() {
        let addr = SolanaAddress::from_base58(SYSTEM_PROGRAM).unwrap();
        assert_eq!(addr.to_base58(), SYSTEM_PROGRAM);
        // System program is all zeros.
        assert_eq!(addr.into_bytes(), [0u8; 32]);
    }

    #[test]
    fn address_serde_json() {
        let addr = SolanaAddress::from_base58(SYSTEM_PROGRAM).unwrap();
        let json = serde_json::to_string(&addr).unwrap();
        assert_eq!(json, format!("\"{}\"", SYSTEM_PROGRAM));
        let back: SolanaAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(addr, back);
    }

    #[test]
    fn address_rejects_wrong_length() {
        // "1" decodes to a single zero byte, far short of 32.
        assert!(matches!(
            SolanaAddress::from_base58("1"),
            Err(SolanaAddressError::WrongLength(_))
        ));
    }

    #[test]
    fn address_rejects_invalid_base58() {
        assert!(matches!(
            SolanaAddress::from_base58("0OIl"),
            Err(SolanaAddressError::Base58(_))
        ));
    }

    #[test]
    fn commitment_default_is_confirmed() {
        assert_eq!(SolanaCommitment::default(), SolanaCommitment::Confirmed);
    }

    #[test]
    fn commitment_serde_round_trip() {
        for level in [
            SolanaCommitment::Processed,
            SolanaCommitment::Confirmed,
            SolanaCommitment::Finalized,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: SolanaCommitment = serde_json::from_str(&json).unwrap();
            assert_eq!(level, back);
        }
        assert_eq!(
            serde_json::to_string(&SolanaCommitment::Confirmed).unwrap(),
            "\"confirmed\""
        );
    }

    #[test]
    fn commitment_from_str() {
        assert_eq!(
            "Finalized".parse::<SolanaCommitment>().unwrap(),
            SolanaCommitment::Finalized
        );
        assert!("foo".parse::<SolanaCommitment>().is_err());
    }
}
