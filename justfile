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

install-native HOME DATA="":
    @if [ "{{DATA}}" != "" ]; then \
        just _install-native {{HOME}} {{DATA}}; \
    else \
        just _install-native {{HOME}} {{HOME}}; \
    fi


_install-native HOME DATA:
    @rm -rf "{{HOME}}"
    @rm -rf "{{DATA}}"
    @mkdir -p "{{HOME}}"
    @mkdir -p "{{DATA}}"
    @cp "./packages/wavs/wavs.toml" "{{HOME}}"
    @cp "./packages/cli/cli.toml" "{{HOME}}"
    @cp "./packages/aggregator/aggregator.toml" "{{HOME}}"
    @cp "./.env.example" "{{HOME}}/.env"
    @if [ "$(uname)" == "Darwin" ]; then \
        sed -i '' -e "s|^# data = \"~/wavs/data\"|data = \"{{DATA}}/wavs\"|" "{{HOME}}/wavs.toml"; \
        sed -i '' -e "s|^# data = \"~/wavs/cli\"|data = \"{{DATA}}/wavs-cli\"|" "{{HOME}}/cli.toml"; \
        sed -i '' -e "s|^# data = \"~/wavs/aggregator\"|data = \"{{DATA}}/wavs-aggregator\"|" "{{HOME}}/aggregator.toml"; \
    else \
        sed -i -e "s|^# data = \"~/wavs/data\"|data = \"{{DATA}}/wavs\"|" "{{HOME}}/wavs.toml"; \
        sed -i -e "s|^# data = \"~/wavs/cli\"|data = \"{{DATA}}/wavs-cli\"|" "{{HOME}}/cli.toml"; \
        sed -i -e "s|^# data = \"~/wavs/aggregator\"|data = \"{{DATA}}/wavs-aggregator\"|" "{{HOME}}/aggregator.toml"; \
    fi
    @cargo install --path ./packages/wavs
    @cargo install --path ./packages/cli
    @cargo install --path ./packages/aggregator
    @echo "Add these variables to your system environment:"
    @echo ""
    @echo "export WAVS_HOME=\"{{HOME}}\""
    @echo "export WAVS_DOTENV=\"{{HOME}}/.env\""

# compile WASI components, places the output in components dir
wasi-build COMPONENT="*":
    @if [ "{{COMPONENT}}" = "*" ]; then \
        rm -f ./examples/target/wasm32-wasip1/release/*.wasm; \
    fi

    @for C in examples/components/{{COMPONENT}}/Cargo.toml; do \
        if [ "{{COMPONENT}}" != "_helpers" ]; then \
            echo "Building WASI component in $(dirname $C)"; \
            ( cd $(dirname $C) && cargo component build --release && cargo fmt); \
        fi; \
    done

    rm -rf {{WASI_OUT_DIR}}
    mkdir -p {{WASI_OUT_DIR}}
    @cp ./examples/target/wasm32-wasip1/release/*.wasm {{WASI_OUT_DIR}}
    @sha256sum -- {{WASI_OUT_DIR}}/*.wasm | tee checksums.txt

# compile solidity contracts (including examples) and copy the ABI to contracts/solidity/abi
# example ABI's will be copied to examples/contracts/solidity/abi
solidity-build CLEAN="":
    @if [ "{{CLEAN}}" = "clean" ]; then \
        rm -rf {{REPO_ROOT}}/out; \
        rm -rf {{REPO_ROOT}}/contracts/solidity/abi; \
        rm -rf {{REPO_ROOT}}/examples/contracts/solidity/abi; \
    fi
    mkdir -p {{REPO_ROOT}}/out
    mkdir -p {{REPO_ROOT}}/contracts/solidity/abi
    mkdir -p {{REPO_ROOT}}/examples/contracts/solidity/abi
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/out --contracts {{REPO_ROOT}}/contracts/solidity;
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/out --contracts {{REPO_ROOT}}/examples/contracts/solidity;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-middleware --out {{REPO_ROOT}}/out;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-middleware/lib/eigenlayer-contracts --out {{REPO_ROOT}}/out;
    @for contract in \
        DelegationManager TransparentUpgradeableProxy ProxyAdmin PauserRegistry AVSDirectory \
        StrategyManager StrategyFactory EigenPodManager RewardsCoordinator EigenPod UpgradeableBeacon StrategyBase \
        ECDSAStakeRegistry LayerToken IStrategy EmptyContract \
        WavsServiceManager WavsServiceAggregator IWavsServiceHandler IWavsServiceManager IWavsServiceAggregator; do \
        cp -r {{REPO_ROOT}}/out/$contract.sol {{REPO_ROOT}}/contracts/solidity/abi; \
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

# e.g. `just cli-exec ./examples/build/components/echo_raw.wasm "hello world"`
cli-exec COMPONENT INPUT:
    @cd packages/cli && cargo run exec --component {{COMPONENT}} --input '{{INPUT}}'

# downloads the latest WIT file from the wavs-wasi repo
download-wit:
    # Create a temporary directory
    rm -rf temp_clone
    mkdir temp_clone
    
    # Clone the specific branch into the temp directory
    git -C temp_clone clone --branch init/wavs-wasi-utils --single-branch https://github.com/Lay3rLabs/wavs-wasi.git
    
    # Copy just the sdk directory to your destination
    mkdir -p sdk
    cp -r temp_clone/wavs-wasi/sdk/* sdk/
    
    # Clean up
    rm -rf temp_clone