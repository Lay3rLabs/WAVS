//! wavs-rig: Bridge library connecting rig-wasi to the WAVS WASI component sandbox.
//!
//! Provides HTTP transport, built-in tools, KV-backed memory, and agent entry-point shim.

pub mod kv_bindings;
pub mod http;
pub mod tools;
pub mod memory;
pub mod agent;
pub mod permissions;
pub mod anthropic;

// Re-export key types for convenience
pub use http::WasiHttpClient;
pub use memory::{WavsMemory, Message};
pub use agent::{WavsAgent, run_agent};
pub use permissions::{HttpPermission, check_http_permission};
