use clap::Parser;
use layer_tests::args::TestArgs;

// for easier debugging without messing with the toml config
// e.g. `cargo run -- --isolated eth-square`
pub fn main() {
    layer_tests::e2e::run(TestArgs::parse());
}
