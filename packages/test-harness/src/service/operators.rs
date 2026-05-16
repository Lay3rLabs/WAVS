//! Operator registration helpers.
//!
//! Re-exports the stable middleware mocks from `utils::test_utils::middleware`
//! and adds a thin `OperatorSet` aggregate type. The mocks deploy real
//! service-manager contracts via Docker images (Eigenlayer / POA), so any test
//! using these helpers requires a working Docker daemon at the in-process tier.
//!
//! For tests that do not need the real middleware (e.g. asserting only that the
//! component WASM produces the expected output), use the `logic` tier — which
//! skips operator registration entirely. The logic tier is exposed by the
//! `service::runner` module.

use alloy_primitives::Address;

pub use utils::test_utils::middleware::evm::{
    EigenlayerMiddleware, EvmMiddleware, EvmMiddlewareType, PoaMiddleware,
    ANVIL_DEPLOYER_ADDRESS, ANVIL_DEPLOYER_KEY,
};
pub use utils::test_utils::middleware::operator::AvsOperator;
pub use utils::test_utils::mock_service_manager::MockEvmServiceManager;

/// A registered set of operators tied to a deployed service-manager.
#[derive(Debug, Clone)]
pub struct OperatorSet {
    pub service_manager: Address,
    pub operators: Vec<AvsOperator>,
}

impl OperatorSet {
    pub fn new(service_manager: Address, operators: Vec<AvsOperator>) -> Self {
        Self {
            service_manager,
            operators,
        }
    }

    /// Number of operators in the set.
    pub fn len(&self) -> usize {
        self.operators.len()
    }

    /// True if no operators are registered.
    pub fn is_empty(&self) -> bool {
        self.operators.is_empty()
    }

    /// Iterate over the operator signers.
    pub fn signers(&self) -> impl Iterator<Item = Address> + '_ {
        self.operators.iter().map(|o| o.signer)
    }
}
