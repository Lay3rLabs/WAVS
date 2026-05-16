//! WAVS service lifecycle: spec, operator registration, runner tiers.
//!
//! - [`config`]: declarative [`ServiceSpec`] builder consumed by all runners.
//! - [`operators`]: middleware mock re-exports and [`OperatorSet`] aggregate.
//! - `runner_inproc` / `runner_subprocess`: tier-specific runners (Steps 5 & 7).

pub mod config;
pub mod operators;
#[cfg(feature = "inproc")]
pub mod runner_inproc;

pub use config::ServiceSpec;
#[cfg(feature = "inproc")]
pub use runner_inproc::InProcRunner;
pub use operators::{
    AvsOperator, EigenlayerMiddleware, EvmMiddleware, EvmMiddlewareType, MockEvmServiceManager,
    OperatorSet, PoaMiddleware, ANVIL_DEPLOYER_ADDRESS, ANVIL_DEPLOYER_KEY,
};
