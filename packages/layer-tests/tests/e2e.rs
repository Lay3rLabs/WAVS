use layer_tests::args::TestArgs;

// This is so we automatically run tests via `cargo test --workspace`
// However, for developer purposes, we can also override and isolate tests
// by running `cargo run` from the `layer-tests` package directory.
// e.g. `cargo run -- --isolated eth-square`
#[test]
fn e2e_tests() {
    layer_tests::e2e::run(TestArgs::default());
}
