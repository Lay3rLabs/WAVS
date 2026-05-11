//! WASM-compatible Anthropic client builder for WAVS agent components.
//!
//! This module provides a clean interface to create an Anthropic completion client
//! using `WasiHttpClient` as the HTTP backend — the only HTTP client available
//! on `wasm32-wasip2`.
//!
//! # Example
//!
//! ```ignore
//! use wavs_rig::anthropic::build_client;
//!
//! let client = build_client(&api_key)?;
//! let agent = client.agent("claude-3-5-haiku-latest").preamble("...").build();
//! let answer = agent.prompt(&prompt).await?;
//! ```

use crate::http::WasiHttpClient;
use anyhow::Result;
use rig::client::ClientBuilder;
use rig::providers::anthropic::client::{AnthropicBuilder, Client};

/// Build an Anthropic `Client` wired to `WasiHttpClient`.
///
/// This is the idiomatic way to create an Anthropic client in a WAVS component.
/// Equivalent to:
/// ```ignore
/// ClientBuilder::<AnthropicBuilder>::default()
///     .api_key(api_key)
///     .http_client(WasiHttpClient::default())
///     .build()?
/// ```
pub fn build_client(api_key: &str) -> Result<Client<WasiHttpClient>> {
    ClientBuilder::<AnthropicBuilder>::default()
        .api_key(api_key)
        .http_client(WasiHttpClient::default())
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))
}
