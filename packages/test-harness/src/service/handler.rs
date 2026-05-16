//! Mock service-handler + service-manager deployment helpers.
//!
//! This module exposes a `MockHandler` that bundles the two reference mock
//! contracts shipped with WAVS — `SimpleServiceManager` (does quorum-weight
//! aggregation and signer sorting) and `SimpleSubmit` (the canonical
//! `IWavsServiceHandler` that decodes `DataWithId` and stores it).
//!
//! Together they let an in-process test exercise the full
//! `trigger → operator → aggregator → sign → submit → assert` lifecycle on
//! local Anvil without booting the WAVS dispatcher or a real EigenLayer/POA
//! stack:
//!
//! ```text
//!   harness                Anvil                    SimpleSubmit
//!  ───────────────────────────────────────────────────────────────
//!   build payload         |                       |
//!   wrap in envelope      |                       |
//!   sign w/ operator key  |                       |
//!   submit envelope ─────►| handleSignedEnvelope ►| _SERVICE_MANAGER.validate()
//!                         |                       | store signedData
//!                         |                       | set validTriggers[id] = true
//!   assert state ◄────────| isValidTriggerId(id)  |
//! ```
//!
//! The `SimpleServiceManager.validate()` path enforces:
//!   - signers.length > 0 and == signatures.length
//!   - referenceBlock < block.number
//!   - signers sorted strictly ascending
//!   - sum(operatorWeights[signers[i]]) >= checkpoint threshold
//!
//! ECDSA recovery is not done by this mock — the production
//! `WavsServiceManager` does ecrecover from the EIP-191 prefixed digest and
//! matches it against the stake registry. The envelope produced by
//! [`crate::envelope::sign_envelope`] is byte-compatible with that production
//! path; see `crate::envelope` tests for the signing-format proofs.

use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use anyhow::{anyhow, Context, Result};

use crate::envelope::{Envelope, SignatureData};

// ---------------------------------------------------------------------------
// sol!-generated bindings for the two mock contracts.
//
// `SimpleServiceManager` is loaded from a vendored artifact under
// `fixtures/contracts/` because forge output (`out/`) is gitignored, so the
// per-package committed copy is the only stable build input.
//
// `SimpleSubmit` is committed under `examples/contracts/solidity/abi/`.
// ---------------------------------------------------------------------------

pub mod manager_abi {
    use alloy_sol_types::sol;
    sol! {
        #[allow(missing_docs, clippy::too_many_arguments)]
        #[sol(rpc)]
        SimpleServiceManager,
        "fixtures/contracts/SimpleServiceManager.json"
    }
}

pub mod handler_abi {
    use alloy_sol_types::sol;
    sol! {
        #[allow(missing_docs, clippy::too_many_arguments)]
        #[sol(rpc)]
        SimpleSubmit,
        "../../examples/contracts/solidity/abi/SimpleSubmit.sol/SimpleSubmit.json"
    }
}

pub use handler_abi::{ISimpleSubmit, ISimpleTrigger, SimpleSubmit};
pub use manager_abi::SimpleServiceManager;

/// Configuration for [`MockHandler::deploy`]. Tests that need stronger
/// quorum semantics override these — defaults match a single-operator harness
/// (one signer with weight 1, threshold 1).
#[derive(Debug, Clone)]
pub struct MockHandlerConfig {
    /// (signer_address, weight) pairs registered with the manager.
    pub operator_weights: Vec<(Address, U256)>,
    /// Threshold weight required for `validate()` to pass.
    pub threshold_weight: U256,
    /// Optional total checkpoint weight (defaults to sum of operator weights).
    pub total_weight: Option<U256>,
}

impl MockHandlerConfig {
    /// One-operator setup: weight 1, threshold 1.
    pub fn single_operator(signer: Address) -> Self {
        Self {
            operator_weights: vec![(signer, U256::from(1))],
            threshold_weight: U256::from(1),
            total_weight: None,
        }
    }

    /// Build from a slice of signer addresses, assigning weight 1 to each and a
    /// quorum threshold equal to `quorum` (must be <= number of signers).
    pub fn quorum_of(signers: &[Address], quorum: usize) -> Result<Self> {
        if signers.is_empty() {
            return Err(anyhow!("at least one signer required"));
        }
        if quorum == 0 || quorum > signers.len() {
            return Err(anyhow!(
                "quorum {quorum} must be in 1..={n}",
                n = signers.len()
            ));
        }
        Ok(Self {
            operator_weights: signers.iter().map(|a| (*a, U256::from(1))).collect(),
            threshold_weight: U256::from(quorum),
            total_weight: None,
        })
    }

    fn computed_total(&self) -> U256 {
        self.total_weight.unwrap_or_else(|| {
            self.operator_weights
                .iter()
                .map(|(_, w)| *w)
                .fold(U256::ZERO, |a, b| a + b)
        })
    }
}

/// A deployed pair of `SimpleServiceManager` + `SimpleSubmit` contracts that
/// together satisfy the `IWavsServiceHandler` + `IWavsServiceManager`
/// contract pair downstream apps target.
pub struct MockHandler<P: Provider + Clone> {
    /// The `SimpleServiceManager` address.
    pub manager: Address,
    /// The `SimpleSubmit` handler address (the address downstream tests submit
    /// envelopes to).
    pub handler: Address,
    /// The provider used to deploy and interact with both contracts.
    pub provider: P,
}

impl<P: Provider + Clone + 'static> MockHandler<P> {
    /// Deploy a fresh `SimpleServiceManager` + `SimpleSubmit` pair against the
    /// provided [`MockHandlerConfig`].
    ///
    /// The deployer is whichever account `provider` uses for its default sender
    /// (typically Anvil's account 0).
    pub async fn deploy(provider: P, config: &MockHandlerConfig) -> Result<Self> {
        let manager_instance = SimpleServiceManager::deploy(provider.clone())
            .await
            .context("deploy SimpleServiceManager")?;
        let manager_addr = *manager_instance.address();
        tracing::debug!(manager = %manager_addr, "deployed SimpleServiceManager");

        // Apply configuration: per-operator weight, threshold, total weight.
        for (signer, weight) in &config.operator_weights {
            manager_instance
                .setOperatorWeight(*signer, *weight)
                .send()
                .await
                .with_context(|| format!("setOperatorWeight for {signer}"))?
                .watch()
                .await?;
        }
        manager_instance
            .setLastCheckpointThresholdWeight(config.threshold_weight)
            .send()
            .await
            .context("setLastCheckpointThresholdWeight")?
            .watch()
            .await?;
        manager_instance
            .setLastCheckpointTotalWeight(config.computed_total())
            .send()
            .await
            .context("setLastCheckpointTotalWeight")?
            .watch()
            .await?;

        let handler_instance = SimpleSubmit::deploy(provider.clone(), manager_addr)
            .await
            .context("deploy SimpleSubmit")?;
        let handler_addr = *handler_instance.address();
        tracing::debug!(handler = %handler_addr, manager = %manager_addr, "deployed SimpleSubmit");

        Ok(Self {
            manager: manager_addr,
            handler: handler_addr,
            provider,
        })
    }

    /// Submit a signed envelope to the handler's `handleSignedEnvelope` entry
    /// point. Returns the transaction receipt.
    pub async fn submit_envelope(
        &self,
        envelope: &Envelope,
        signature: &SignatureData,
    ) -> Result<alloy_rpc_types_eth::TransactionReceipt> {
        crate::envelope::submit_envelope(&self.provider, self.handler, envelope, signature).await
    }

    /// Convenience: poll `isValidTriggerId` on the handler for a given trigger.
    pub async fn is_valid_trigger(&self, trigger_id: u64) -> Result<bool> {
        let h = SimpleSubmit::new(self.handler, &self.provider);
        let ok = h
            .isValidTriggerId(trigger_id.into())
            .call()
            .await
            .with_context(|| format!("isValidTriggerId({trigger_id})"))?;
        Ok(ok)
    }
}
