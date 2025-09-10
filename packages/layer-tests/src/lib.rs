#![cfg(test)] // Everything is test-only
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::result_large_err)]

pub mod args;
pub mod config;
pub mod deployment;
pub mod e2e;
pub mod example_cosmos_client;
pub mod example_evm_client;

use crate::args::TestArgs;
use utils::context::AppContext;
#[test]
fn e2e_tests() {
    let ctx = AppContext::new();
    e2e::run(TestArgs::default(), ctx);
}
