//! Built-in WAVS tools for rig agents.
//!
//! Five tool structs implementing rig's `Tool` trait:
//! - `KvGetTool` — read from wasi:keyvalue store
//! - `KvSetTool` — write to wasi:keyvalue store
//! - `HttpFetchTool` — HTTP requests via wstd::http
//! - `EvmQueryTool` — read-only eth_call via JSON-RPC over HTTP
//! - `LogTool` — structured logging

pub mod kv;
pub mod http;
pub mod evm;
pub mod log;

pub use kv::{KvGetTool, KvSetTool};
pub use self::http::HttpFetchTool;
pub use evm::EvmQueryTool;
pub use self::log::LogTool;
