use alloy_primitives::FixedBytes;
use wavs_types::{EventId, EventOrder, Submit};

use super::address::rand_address_evm;

pub fn mock_eigen_submit() -> Submit {
    Submit::evm_contract("evm".try_into().unwrap(), rand_address_evm(), None)
}

pub fn mock_event_id() -> EventId {
    FixedBytes::new([0; 20]).into()
}

pub fn mock_event_order() -> EventOrder {
    FixedBytes::new([0; 12]).into()
}
