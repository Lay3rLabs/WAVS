
# run unit test
test-unit:
    cargo test --lib

# mock integration
test-mock-integration:
    cargo test

# on-chain integration test
test-wavs-e2e-ethereum:
    cargo test -p wavs --features e2e_tests_ethereum e2e_tests

update-submodules:
    git submodule update --init --recursive

build:
    cargo build --release


lint:
    cargo fmt --all
