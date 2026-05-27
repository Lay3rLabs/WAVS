//! WAVS service lifecycle: spec, operator registration, runner tiers.
//!
//! - [`config`]: declarative [`ServiceSpec`] builder consumed by all runners.
//! - [`operators`]: middleware mock re-exports and [`OperatorSet`] aggregate.
//! - [`handler`]: deployable `SimpleServiceManager` + `SimpleSubmit` mocks
//!   exposing the on-chain `submit` stage so harness tests can assert against
//!   contract state.
//! - `runner_inproc` / `runner_subprocess`: tier-specific runners.

pub mod config;
pub mod handler;
pub mod operators;
#[cfg(feature = "inproc")]
pub mod runner_inproc;
#[cfg(feature = "subprocess")]
pub mod runner_subprocess;

pub use config::ServiceSpec;
pub use handler::{MockHandler, MockHandlerConfig, SimpleServiceManager, SimpleSubmit};
pub use operators::{
    AvsOperator, EigenlayerMiddleware, EvmMiddleware, EvmMiddlewareType, MockEvmServiceManager,
    OperatorSet, PoaMiddleware, ANVIL_DEPLOYER_ADDRESS, ANVIL_DEPLOYER_KEY,
};
#[cfg(feature = "inproc")]
pub use runner_inproc::{InProcRunner, RunnerAggregatorAction, RunnerSubmitAction};
#[cfg(feature = "subprocess")]
pub use runner_subprocess::{SubprocessConfig, SubprocessRunner};
