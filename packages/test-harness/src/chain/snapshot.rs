//! EVM snapshot / revert primitives.
//!
//! Provides explicit [`snapshot`] / [`revert`] functions plus a [`SnapshotGuard`] RAII
//! handle. The guard logs a warning if dropped without an explicit revert — async-drop
//! support is not stable, so consumers are expected to call `.revert(&provider).await`
//! before letting the guard go out of scope.

use alloy_network::Ethereum;
use alloy_primitives::U256;
use alloy_provider::{ext::AnvilApi, Provider};
use anyhow::Result;

/// Take a chain snapshot. Returns the snapshot id.
pub async fn snapshot(provider: &(impl Provider + AnvilApi<Ethereum>)) -> Result<U256> {
    let id = provider.anvil_snapshot().await?;
    tracing::debug!(snapshot_id = %id, "evm_snapshot");
    Ok(id)
}

/// Revert the chain to a previous snapshot. Returns `true` on success.
pub async fn revert(
    provider: &(impl Provider + AnvilApi<Ethereum>),
    id: U256,
) -> Result<bool> {
    let ok = provider.anvil_revert(id).await?;
    tracing::debug!(snapshot_id = %id, ok, "evm_revert");
    Ok(ok)
}

/// RAII guard around a snapshot id.
///
/// Call [`SnapshotGuard::revert`] explicitly before the guard drops. The guard cannot
/// auto-revert in `Drop` because that would require blocking on an async call; instead
/// it logs a warning if dropped without being consumed.
pub struct SnapshotGuard {
    id: Option<U256>,
}

impl SnapshotGuard {
    /// Take a snapshot and return a guard wrapping the id.
    pub async fn take(provider: &(impl Provider + AnvilApi<Ethereum>)) -> Result<Self> {
        let id = snapshot(provider).await?;
        Ok(Self { id: Some(id) })
    }

    /// Revert to the captured snapshot. Consumes the guard.
    pub async fn revert(
        mut self,
        provider: &(impl Provider + AnvilApi<Ethereum>),
    ) -> Result<bool> {
        let id = self.id.take().expect("guard already consumed");
        revert(provider, id).await
    }

    /// The underlying snapshot id, if not yet consumed.
    pub fn id(&self) -> Option<U256> {
        self.id
    }
}

impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            tracing::warn!(
                snapshot_id = %id,
                "SnapshotGuard dropped without revert — chain state may carry across tests"
            );
        }
    }
}
