/// Decode a given `log_data` into a typed event `T`.
///
/// The log data comes from the WIT bindings at https://wa.dev/wavs:operator#layer-types-evm-event-log-data
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
            .map(|t| $crate::evm::alloy_primitives::FixedBytes::<32>::from_slice(t))
            .collect();

        $crate::evm::event::decode_event_log_data_raw(topics, $log_data.data.into())
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

    T::decode_log_data(&log_data).map_err(|e| anyhow!("failed to decode event: {}", e))
}

#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use alloy_sol_types::sol;
    use alloy_sol_types::SolEvent;

    // Define a dummy event for testing
    sol! {
        event MyEvent(uint256 indexed id, address user, uint256 amount);
    }

    // The log data comes from the WIT bindings at https://wa.dev/wavs:operator#layer-types-evm-event-log-data
    struct EvmEventLogData {
        topics: Vec<Vec<u8>>,
        data: Vec<u8>,
    }

    #[test]
    fn test_decode_event_log_data() {
        // Create example topic and data for MyEvent
        let id = U256::from(42u128);
        let user = [0x11u8; 20];
        let amount = U256::from(1000u128);

        // This is normally done via real encoding, but for the test we can simulate
        let event = MyEvent {
            id,
            user: user.into(),
            amount,
        };

        // Encode the event to log data
        let log = event.encode_log_data();
        let log = EvmEventLogData {
            topics: log.topics().iter().map(|t| t.as_slice().to_vec()).collect(),
            data: log.data.to_vec(),
        };

        // Now decode using our macro
        let decoded_event: MyEvent = decode_event_log_data!(log).expect("decode failed");

        // Check values match
        assert_eq!(decoded_event.id, id);
        assert_eq!(decoded_event.user.0, user);
        assert_eq!(decoded_event.amount, amount);
    }
}
