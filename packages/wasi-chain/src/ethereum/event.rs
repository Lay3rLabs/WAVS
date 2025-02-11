/*************  ✨ Codeium Command 🌟  *************/
/// Decode a given `log_data` into a typed event `T`.
///
/// The log data comes from the WIT bindings at https://wa.dev/wavs:worker#layer-types-eth-event-log-data
/// * `topics` should be a Vec<Vec<u8>>`.
/// * `data` should be a `Vec<u8>`.
///
/// `T` should be a type that implements `SolEvent`.
///
/// # Example
///
/// ```ignore
/// let event:MyEvent = decode_event_log_data!(log_data)?;
/// ```
///
#[macro_export]
macro_rules! decode_event_log_data {
    ($log_data:expr) => {{
        let topics = $log_data
            .topics
            .iter()
            .map(|t| $crate::ethereum::alloy_primitives::FixedBytes::<32>::from_slice(t))
            .collect();

        $crate::ethereum::event::decode_event_log_data_raw(topics, $log_data.data.into())
    }};
}

use alloy_primitives::{Bytes, FixedBytes, LogData};
use anyhow::{anyhow, Result};

pub fn decode_event_log_data_raw<T: alloy_sol_types::SolEvent>(
    topics: Vec<FixedBytes<32>>,
    data: Bytes,
) -> Result<T> {
    let log_data =
        LogData::new(topics, data).ok_or_else(|| anyhow!("failed to create log data"))?;

    T::decode_log_data(&log_data, false).map_err(|e| anyhow!("failed to decode event: {}", e))
}
