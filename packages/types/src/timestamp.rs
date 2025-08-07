use std::{num::ParseIntError, str::FromStr};

use anyhow::Result;
#[cfg(feature = "clock")]
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[repr(transparent)]
#[derive(
    Debug, Hash, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ToSchema,
)]
pub struct Timestamp(u64);

impl Timestamp {
    // Create a new Timestamp directly from nanoseconds
    pub fn from_nanos(nanos: u64) -> Self {
        Timestamp(nanos)
    }

    // Create a new Timestamp from DateTime<Utc>
    #[cfg(feature = "clock")]
    pub fn from_datetime(dt: DateTime<Utc>) -> Result<Self> {
        let nanos = dt
            .timestamp_nanos_opt()
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?;

        if nanos < 0 {
            return Err(anyhow::anyhow!(
                "Timestamp cannot represent dates before 1970"
            ));
        }

        Ok(Timestamp(nanos as u64))
    }

    #[cfg(feature = "clock")]
    pub fn into_datetime(self) -> DateTime<Utc> {
        Utc.timestamp_nanos(self.0 as i64)
    }

    // Get the nanosecond value
    pub fn as_nanos(&self) -> u64 {
        self.0
    }

    // Create from current time
    #[cfg(feature = "clock")]
    pub fn now() -> Self {
        // Current time is always after 1970, so this unwrap is safe
        Self::from_datetime(Utc::now()).expect("Current time should always be valid")
    }
}

// Define FromStr for to enable parsing from command line strings
impl FromStr for Timestamp {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let nanos: u64 = s.parse()?;
        Ok(Timestamp::from_nanos(nanos))
    }
}
