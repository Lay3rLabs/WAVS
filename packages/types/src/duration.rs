use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Serialize,
    Deserialize,
    Clone,
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
