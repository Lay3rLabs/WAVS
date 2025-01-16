use crate::bindings::compat::EthEventLogData;
use alloy_primitives::FixedBytes;
use anyhow::{anyhow, Result};

pub fn decode_event_log_data<T: alloy_sol_types::SolEvent>(log_data: EthEventLogData) -> Result<T> {
    let topics = log_data
        .topics
        .iter()
        .map(|t| FixedBytes::<32>::from_slice(t))
        .collect();
    let log_data = alloy_primitives::LogData::new(topics, log_data.data.into())
        .ok_or_else(|| anyhow!("failed to create log data"))?;

    T::decode_log_data(&log_data, false).map_err(|e| anyhow!("failed to decode event: {}", e))
}
