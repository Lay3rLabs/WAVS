
# run unit test
test-unit:
    cargo test --lib

# mock integration
test-mock-integration:
    cargo test

# on-chain integration test
test-e2e:
    cargo test --features e2e_tests,e2e_tests_ethereum e2e_tests

update-submodules:
    git submodule update --init --recursive

build:
    cargo build --release

run-build:
    ( cd ./packages/wavs; ../../target/release/wavs )

lint:
    cargo fmt --all
