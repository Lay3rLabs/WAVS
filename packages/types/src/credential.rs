use std::{ops::Deref, str::FromStr};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A wrapper around a credential string that zeroizes on drop
/// This can be used to store sensitive information such as mnemonics, http auth tokens, or private keys
#[derive(
    Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Zeroize, ZeroizeOnDrop, ToSchema,
)]
#[serde(transparent)]
pub struct Credential(String);

impl Credential {
    /// Create a new Credential from a string
    pub fn new(credential: String) -> Self {
        Self(credential)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Credential {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for Credential {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl FromStr for Credential {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl std::fmt::Display for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
