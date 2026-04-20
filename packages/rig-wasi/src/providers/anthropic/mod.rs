//! Anthropic API client and Rig integration
//!
//! # Example
//! ```
//! use rig::providers::anthropic;
//!
//! let client = anthropic::Client::new("YOUR_API_KEY");
//!
//! let sonnet = client.completion_model(anthropic::completion::CLAUDE_SONNET_4_6);
//! ```

pub mod client;
pub mod completion;
pub mod decoders;
pub mod model_listing;
// P7: Streaming uses SSE which requires sse::GenericEventSource and reqwest feature.
// Gate out on WASM and when reqwest is not enabled.
// Non-streaming completions (the common case for WASI agents) work without this module.
#[cfg(all(not(target_family = "wasm"), feature = "reqwest"))]
pub mod streaming;

pub use client::{Client, ClientBuilder};
