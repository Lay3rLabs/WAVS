//! wavs-rig: Bridge library connecting rig-wasi to the WAVS WASI component sandbox.
//!
//! Provides HTTP transport, built-in tools, KV-backed memory, and agent entry-point shim.

pub mod http;
pub mod tools;
pub mod memory;
pub mod agent;
pub mod permissions;

// Re-export key rig types for convenience
pub use rig::agent::Agent;
pub use rig::completion::ToolDefinition;
pub use rig::tool::Tool;
