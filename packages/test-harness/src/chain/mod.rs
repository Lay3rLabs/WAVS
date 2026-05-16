//! Chain control primitives: local Anvil, pinned forks, snapshots, impersonation,
//! time control, and sanitized logging.

pub mod anvil;
#[cfg(feature = "fork")]
pub mod fork;
pub mod impersonate;
pub mod logging;
pub mod snapshot;
pub mod time;

pub use anvil::{safe_spawn_anvil, spawn_local};
#[cfg(feature = "fork")]
pub use fork::{spawn_fork, ForkOptions, DEFAULT_FORK_RPC_ENV};
pub use impersonate::{
    enable_auto_impersonate, impersonate_funded, set_balance, stop_impersonating, ONE_ETH,
};
pub use logging::{redact_key, redact_url};
pub use snapshot::{revert, snapshot, SnapshotGuard};
pub use time::{increase_time, mine_blocks, set_automine, set_next_block_timestamp};
