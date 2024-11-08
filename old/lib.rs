//! A CLI for Wasmatic.

//#![deny(missing_docs)]

mod app;
pub mod commands;
mod config;
mod cron_bindings;
mod digest;
mod lock;
mod operator;
mod queue;
mod storage;
mod task_bindings;
