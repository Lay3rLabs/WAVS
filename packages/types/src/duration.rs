use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    bincode::Encode,
    bincode::Decode,
    ToSchema,
)]
pub struct Duration {
    pub secs: u64,
}

impl From<Duration> for std::time::Duration {
    fn from(d: Duration) -> Self {
        std::time::Duration::from_secs(d.secs)
    }
}

impl From<std::time::Duration> for Duration {
    fn from(d: std::time::Duration) -> Self {
        Duration { secs: d.as_secs() }
    }
}
