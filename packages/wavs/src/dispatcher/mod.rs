mod core;
mod generic;

pub use core::*;
pub use generic::*;

pub const TRIGGER_CHANNEL_SIZE: usize = 100;
pub const ENGINE_CHANNEL_SIZE: usize = 20;
pub const SUBMISSION_CHANNEL_SIZE: usize = 20;
