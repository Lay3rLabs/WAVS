use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// Returns the number of seconds since the Unix epoch.
    pub fn as_seconds(self) -> u64 {
        self.0
    }
}
