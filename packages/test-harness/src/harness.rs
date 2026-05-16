//! Top-level [`TestHarness`] — bundles a chain provider, Anvil instance, and an
//! [`InProcRunner`] into one value tests can pass around.
//!
//! For maximum flexibility, prefer composing the primitives directly:
//!
//! ```ignore
//! use wavs_test_harness::{chain, service::{InProcRunner, ServiceSpec}};
//!
//! let (provider, anvil) = chain::spawn_local().await?;
//! let runner = InProcRunner::from_spec(&ServiceSpec::new()
//!     .component_wasm("examples/build/components/echo_data.wasm")
//!     .aggregator_wasm("examples/build/components/simple_aggregator.wasm"))?;
//! let outputs = runner.run_component(b"hello".to_vec()).await?;
//! ```
//!
//! The harness adds a convenience constructor that ties them together. Apps that
//! want more control should reach for the underlying primitives.

use alloy_network::Ethereum;
use alloy_node_bindings::AnvilInstance;
use alloy_provider::{ext::AnvilApi, Provider};
use anyhow::Result;

#[cfg(feature = "inproc")]
use crate::service::{InProcRunner, ServiceSpec};

/// One-stop value bundling chain state and the in-process runner.
///
/// Drop semantics: the held [`AnvilInstance`] kills the underlying Anvil
/// subprocess when the harness goes out of scope.
#[cfg(feature = "inproc")]
pub struct TestHarness<P: Provider + AnvilApi<Ethereum> + Clone> {
    pub provider: P,
    pub anvil: AnvilInstance,
    pub runner: InProcRunner,
}

#[cfg(feature = "inproc")]
impl<P: Provider + AnvilApi<Ethereum> + Clone> TestHarness<P> {
    /// Build a harness from already-spawned chain primitives and a validated spec.
    pub fn new(provider: P, anvil: AnvilInstance, spec: &ServiceSpec) -> Result<Self> {
        let runner = InProcRunner::from_spec(spec)?;
        Ok(Self {
            provider,
            anvil,
            runner,
        })
    }

    /// Mine `count` blocks on the underlying chain. Delegates to
    /// [`crate::chain::mine_blocks`].
    pub async fn mine_blocks(&self, count: u64) -> Result<()> {
        crate::chain::mine_blocks(&self.provider, count).await
    }

    /// Take a snapshot of chain state. The returned guard reverts on
    /// explicit `.revert(provider)`, or warns on `Drop`.
    pub async fn snapshot(&self) -> Result<crate::chain::SnapshotGuard> {
        crate::chain::SnapshotGuard::take(&self.provider).await
    }
}
