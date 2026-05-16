//! Local Anvil spawn primitives.
//!
//! The harness reuses [`safe_spawn_anvil`] from `utils::test_utils` (which retries
//! on port collisions). This module re-exports it and adds a convenience that also
//! returns a connected provider so callers don't have to wire up `ProviderBuilder`.

use alloy_network::Ethereum;
use alloy_provider::{ext::AnvilApi, Provider, ProviderBuilder};
use anyhow::Result;

#[allow(unused_imports)]
pub use utils::test_utils::anvil::{safe_spawn_anvil, safe_spawn_anvil_extra};

use alloy_node_bindings::AnvilInstance;

/// Spawn a fresh local Anvil and return a connected provider alongside the instance.
///
/// Port collisions are handled automatically (see [`safe_spawn_anvil`]).
pub async fn spawn_local() -> Result<(impl Provider + AnvilApi<Ethereum> + Clone, AnvilInstance)> {
    let anvil = safe_spawn_anvil();
    #[allow(deprecated)]
    let provider = ProviderBuilder::new().on_http(anvil.endpoint_url());
    Ok((provider, anvil))
}
