SUDO := if `groups | grep -q docker > /dev/null 2>&1 && echo true || echo false` == "true" { "" } else { "sudo" }
WASI_OUT_DIR := "./components"
REPO_ROOT := `git rev-parse --show-toplevel`
DOCKER_WAVS_ID := `docker ps | grep wavs | awk '{print $1}'`

help:
  just --list

# builds wavs
docker-build TAG="local":
    {{SUDO}} docker build . -t ghcr.io/lay3rlabs/wavs:{{TAG}}

# run wavs:latest
docker-run:
    {{SUDO}} docker run --rm ghcr.io/lay3rlabs/wavs:latest

# stop the running wavs container
docker-stop:
    if [ "{{DOCKER_WAVS_ID}}" != "" ]; then \
        {{SUDO}} docker kill {{DOCKER_WAVS_ID}}; \
        echo "Stopped container {{DOCKER_WAVS_ID}}"; \
    else \
        echo "No container running"; \
    fi

# compile WASI components, places the output in components dir
wasi-build COMPONENT="*":
    rm -f ./examples/target/wasm32-wasip1/release/*.wasm 
    rm -rf ./components/
    mkdir -p ./components/
    @for C in examples/{{COMPONENT}}/Cargo.toml; do \
        echo "Building WASI component in $(dirname $C)"; \
        `cd $(dirname $C); cargo component build --release; cargo fmt;`; \
    done

    @cp ./examples/target/wasm32-wasip1/release/*.wasm {{WASI_OUT_DIR}}
    @sha256sum -- {{WASI_OUT_DIR}}/*.wasm | tee checksums.txt

# compile solidity contracts and copy the ABI to contracts/abi
solidity-build:
    mkdir -p {{REPO_ROOT}}/out
    mkdir -p {{REPO_ROOT}}/contracts/abi
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/out;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-middleware --out {{REPO_ROOT}}/out;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-middleware/lib/eigenlayer-contracts --out {{REPO_ROOT}}/out;
    @for contract in \
        DelegationManager TransparentUpgradeableProxy ProxyAdmin PauserRegistry AVSDirectory StrategyManager StrategyFactory EigenPodManager RewardsCoordinator EigenPod UpgradeableBeacon StrategyBase \
        ECDSAStakeRegistry LayerToken IStrategy LayerServiceManager LayerTrigger EmptyContract; do \
        cp -r {{REPO_ROOT}}/out/$contract.sol {{REPO_ROOT}}/contracts/abi; \
    done

# on-chain integration test
test-wavs-e2e-ethereum:
    RUST_LOG=debug,alloy_rpc=off,alloy_provider=off,wasmtime=off,cranelift=off,hyper_util=off cargo test -p wavs --features e2e_tests_ethereum_baseline e2e_tests

update-submodules:
    git submodule update --init --recursive

lint:
    cargo fmt --all -- --check
    cargo fix --allow-dirty --allow-staged
    cargo clippy --all-targets -- -D warnings

# waiting on: https://github.com/casey/just/issues/626
start-all:
  #!/bin/bash -eux
  just start-anvil &
  just start-aggregator &
  just start-wavs &
  trap 'kill $(jobs -pr)' EXIT
  wait

start-wavs:
    cd packages/wavs && cargo run

start-aggregator:
    cd packages/aggregator && cargo run

start-anvil:
    anvil

cli-deploy-core:
    @cd packages/cli && cargo run --quiet deploy-core

# e.g. just cli-deploy-service ./components/eth_trigger_square.wasm [SERVICE_MANAGER_ADDR]
cli-deploy-service COMPONENT SERVICE_MANAGER_ADDR="":
    @if [ "{{SERVICE_MANAGER_ADDR}}" == "" ]; then \
        cd packages/cli && cargo run --quiet deploy-service --component "{{COMPONENT}}"; \
    else \
        cd packages/cli && cargo run --quiet deploy-service --component "{{COMPONENT}}" --service-manager '{{SERVICE_MANAGER_ADDR}}'; \
    fi

# e.g. `just cli-add-task 01942c3a85987e209520df364b3ba85b 7B2278223A20337D` or `{\"x\":2}`
cli-add-task SERVICE_ID INPUT:
    @cd packages/cli && cargo run --quiet add-task --service-id {{SERVICE_ID}} --input '{{INPUT}}'

# e.g. `just cli-exec ./components/eth_trigger_square.wasm {\"x\":2}`
cli-exec COMPONENT INPUT:
    @cd packages/cli && cargo run exec --component {{COMPONENT}} --input '{{INPUT}}'
