// Re-export worker bindings as the main world bindings for backward compatibility
pub use crate::worker::bindings::world::*;

// Re-export aggregator bindings
pub mod aggregator {
    pub use crate::aggregator::bindings::world::*;
}
