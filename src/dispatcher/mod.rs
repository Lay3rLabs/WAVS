mod core;
pub mod mock;
// I just leave this for the older version to not break anything,
// this should go away and use core instead
mod placeholder;

// pub use core::Dispatcher;
pub use placeholder::Dispatcher;
