use clap::Parser;
use layer_tests::args::TestArgs;
use utils::context::AppContext;

// for easier debugging without messing with the toml config
// e.g. `cargo run -- --isolated evm-square`
pub fn main() {
    let ctx = AppContext::new();
    layer_tests::e2e::run(TestArgs::parse(), ctx);
}
