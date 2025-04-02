use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[repr(transparent)]
#[derive(Debug, Hash, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    // Create a new Timestamp directly from nanoseconds
    pub fn from_nanos(nanos: u64) -> Self {
        Timestamp(nanos)
    }

    // Create a new Timestamp from DateTime<Utc>
    pub fn from_datetime(dt: DateTime<Utc>) -> Result<Self> {
        let nanos = dt
            .timestamp_nanos_opt()
            .ok_or_else(|| anyhow!("Invalid timestamp"))?;

        if nanos < 0 {
            return Err(anyhow!("Timestamp cannot represent dates before 1970"));
        }

        Ok(Timestamp(nanos as u64))
    }

    // Get the nanosecond value
    pub fn as_nanos(&self) -> u64 {
        self.0
    }

    // Create from current time
    pub fn now() -> Self {
        // Current time is always after 1970, so this unwrap is safe
        Self::from_datetime(Utc::now()).expect("Current time should always be valid")
    }
}
