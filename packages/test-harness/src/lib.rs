//! # wavs-test-harness
//!
//! Reusable integration test harness for WAVS apps.
//!
//! Today this crate is a scaffold — only the public module layout exists. Subsequent
//! commits will fill in:
//!
//! - [`chain`]: local Anvil + pinned-fork support, snapshot/revert, impersonation, time control.
//! - [`fixtures`]: TOML chain profiles and typed address lookup.
//! - [`service`]: WAVS service lifecycle runner (in-process and subprocess tiers).
//! - [`lifecycle`]: trigger emission, quorum/submission waiters, contract assertions.
//! - [`envelope`]: signed-envelope helpers verified against downstream handlers.
//!
//! See `README.md` for the tier matrix, fixture file format, and downstream-consumer example.
//!
//! Tracks issue [Lay3rLabs/WAVS#1147](https://github.com/Lay3rLabs/WAVS/issues/1147).

pub mod chain;
pub mod envelope;
pub mod fixtures;
pub mod lifecycle;
pub mod service;
