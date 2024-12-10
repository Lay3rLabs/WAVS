# on-chain integration test
test-wavs-e2e-ethereum:
    cargo test -p wavs --features e2e_tests_ethereum e2e_tests

update-submodules:
    git submodule update --init --recursive

lint:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings
