//! Local Anvil spawn primitives.
//!
//! The harness reuses [`safe_spawn_anvil`] from `utils::test_utils` (which retries
//! on port collisions). This module re-exports it and adds a convenience that also
//! returns a connected provider so callers don't have to wire up `ProviderBuilder`.

use alloy_network::Ethereum;
use alloy_provider::{ext::AnvilApi, DynProvider, Provider, ProviderBuilder};
use alloy_signer_local::PrivateKeySigner;
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

/// Spawn a fresh local Anvil with a wallet-aware provider.
///
/// The returned provider is signed by Anvil account 0 (the default deployer)
/// so calls to `Contract::deploy(provider, ...)` and other state-changing
/// transactions work out of the box. Returns `(provider, anvil, deployer_signer)`.
///
/// Use this when you need to deploy contracts on Anvil from within the harness —
/// the plain [`spawn_local`] is fine for read-only or impersonated work.
pub async fn spawn_local_with_deployer() -> Result<(DynProvider, AnvilInstance, PrivateKeySigner)> {
    let anvil = safe_spawn_anvil();
    let deployer = PrivateKeySigner::from_signing_key(anvil.keys()[0].clone().into());
    let provider = ProviderBuilder::new()
        .wallet(deployer.clone())
        .connect_http(anvil.endpoint().parse()?);
    Ok((DynProvider::new(provider), anvil, deployer))
}
