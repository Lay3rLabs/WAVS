SUDO := if `groups | grep -q docker > /dev/null 2>&1 && echo true || echo false` == "true" { "" } else { "sudo" }
TAG := env_var_or_default("TAG", "")
WASI_OUT_DIR := "./examples/build/components"
COSMWASM_OUT_DIR := "./examples/build/contracts"
REPO_ROOT := `git rev-parse --show-toplevel`
DOCKER_WAVS_ID := `docker ps | grep wavs | awk '{print $1}'`
ARCH := `uname -m`

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
    @if [ "{{COMPONENT}}" = "*" ]; then \
        rm -f ./examples/target/wasm32-wasip1/release/*.wasm; \
    fi

    @for C in examples/components/{{COMPONENT}}/Cargo.toml; do \
        if [ "{{COMPONENT}}" != "_helpers" ]; then \
            echo "Building WASI component in $(dirname $C)"; \
            ( cd $(dirname $C) && cargo component build --release && cargo fmt ); \
        fi; \
    done

    rm -rf {{WASI_OUT_DIR}}
    mkdir -p {{WASI_OUT_DIR}} 
    @cp ./examples/target/wasm32-wasip1/release/*.wasm {{WASI_OUT_DIR}}
    @sha256sum -- {{WASI_OUT_DIR}}/*.wasm | tee checksums.txt

# compile solidity contracts (including examples) and copy the ABI to sdk/solidity/contracts/abi
# example ABI's will be copied to examples/contracts/solidity/abi
solidity-build CLEAN="":
    @if [ "{{CLEAN}}" = "clean" ]; then \
        rm -rf {{REPO_ROOT}}/out; \
        rm -rf {{REPO_ROOT}}/sdk/solidity/contracts/abi; \
        rm -rf {{REPO_ROOT}}/examples/contracts/solidity/abi; \
    fi
    mkdir -p {{REPO_ROOT}}/out
    mkdir -p {{REPO_ROOT}}/sdk/solidity/contracts/abi
    mkdir -p {{REPO_ROOT}}/examples/contracts/solidity/abi
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/out --contracts {{REPO_ROOT}}/sdk/solidity/contracts;
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/out --contracts {{REPO_ROOT}}/examples/contracts/solidity;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-middleware --out {{REPO_ROOT}}/out;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-middleware/lib/eigenlayer-contracts --out {{REPO_ROOT}}/out;
    @for contract in \
        DelegationManager TransparentUpgradeableProxy ProxyAdmin PauserRegistry AVSDirectory StrategyManager StrategyFactory EigenPodManager RewardsCoordinator EigenPod UpgradeableBeacon StrategyBase \
        ECDSAStakeRegistry LayerToken IStrategy LayerServiceManager ILayerTrigger EmptyContract; do \
        cp -r {{REPO_ROOT}}/out/$contract.sol {{REPO_ROOT}}/sdk/solidity/contracts/abi; \
    done
    cp -r {{REPO_ROOT}}/out/SimpleTrigger.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/ISimpleTrigger.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/SimpleSubmit.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/ISimpleSubmit.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/

# compile cosmwasm example contracts
cosmwasm-build:
    rm -rf {{COSMWASM_OUT_DIR}}
    mkdir -p {{COSMWASM_OUT_DIR}}
    @if [ "{{ARCH}}" = "arm64" ]; then \
      docker run --rm \
        -v "{{REPO_ROOT}}:/code" \
        --mount type=volume,source="layer_wavs_cache",target=/target \
        --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
        cosmwasm/optimizer-arm64:0.16.1 ./examples; \
    else \
      docker run --rm \
        -v "{{REPO_ROOT}}:/code" \
        --mount type=volume,source="layer_wavs_cache",target=/target \
        --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
        cosmwasm/optimizer:0.16.1 ./examples; \
    fi
    cp ./artifacts/*.wasm {{COSMWASM_OUT_DIR}}

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