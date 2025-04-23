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
        rm -f ./target/wasm32-wasip1/release/*.wasm; \
    fi

    @for C in examples/components/{{COMPONENT}}/Cargo.toml; do \
        if [ "{{COMPONENT}}" != "_helpers" ]; then \
            echo "Building WASI component in $(dirname $C)"; \
            ( cd $(dirname $C) && cargo component build --release && cargo fmt); \
        fi; \
    done

    rm -rf {{WASI_OUT_DIR}}
    mkdir -p {{WASI_OUT_DIR}}
    @cp ./target/wasm32-wasip1/release/*.wasm {{WASI_OUT_DIR}}
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
    cp -r {{REPO_ROOT}}/out/IWavsServiceHandler.sol {{REPO_ROOT}}/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/IWavsServiceManager.sol {{REPO_ROOT}}/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/SimpleTrigger.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/ISimpleTrigger.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/SimpleSubmit.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/ISimpleSubmit.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/SimpleServiceManager.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/

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
download-wit branch="main":
    # Create a temporary directory
    rm -rf temp_clone
    mkdir temp_clone
    
    # Clone the specific branch into the temp directory
    git -C temp_clone clone --depth=1 --branch {{branch}} --single-branch https://github.com/Lay3rLabs/wavs-wasi.git
    
    # Clear existing content and create wit directory
    rm -rf wit
    mkdir -p wit
    
    # Copy just the wit directory and lock file from the cloned repo
    cp -r temp_clone/wavs-wasi/wit/* wit/
    cp -r temp_clone/wavs-wasi/wkg.lock wkg.lock
    
    # Clean up
    rm -rf temp_clone

# downloads the latest solidity repo
download-solidity branch="dev":
    # Create a temporary directory
    rm -rf temp_clone
    mkdir temp_clone
    
    # Clone the specific branch into the temp directory
    git -C temp_clone clone --depth=1 --branch {{branch}} --single-branch https://github.com/Lay3rLabs/wavs-middleware.git 
    
    # Clear existing content and create solidity directory
    rm -rf contracts/solidity
    rm -rf examples/contracts/solidity
    mkdir -p contracts/solidity/interfaces
    mkdir -p examples/contracts/solidity/interfaces
    
    # Copy just what we need 
    cp temp_clone/wavs-middleware/contracts/interfaces/IWavsServiceHandler.sol contracts/solidity/interfaces/IWavsServiceHandler.sol
    cp temp_clone/wavs-middleware/contracts/interfaces/IWavsServiceManager.sol contracts/solidity/interfaces/IWavsServiceManager.sol

    # and, for examples
    cp temp_clone/wavs-middleware/contracts/interfaces/IWavsServiceHandler.sol examples/contracts/solidity/interfaces/IWavsServiceHandler.sol
    cp temp_clone/wavs-middleware/contracts/interfaces/IWavsServiceManager.sol examples/contracts/solidity/interfaces/IWavsServiceManager.sol
    cp temp_clone/wavs-middleware/contracts/interfaces/ISimpleSubmit.sol examples/contracts/solidity/interfaces/ISimpleSubmit.sol
    cp temp_clone/wavs-middleware/contracts/interfaces/ISimpleTrigger.sol examples/contracts/solidity/interfaces/ISimpleTrigger.sol
    cp temp_clone/wavs-middleware/contracts/src/SimpleTrigger.sol examples/contracts/solidity/src/SimpleTrigger.sol
    cp temp_clone/wavs-middleware/contracts/src/SimpleSubmit.sol examples/contracts/solidity/src/SimpleSubmit.sol
    cp temp_clone/wavs-middleware/contracts/src/SimpleServiceManager.sol examples/contracts/solidity/src/SimpleServiceManager.sol
    
    # Clean up
    rm -rf temp_clone

wasi-publish component="*" version="0.4.0-alpha.5":
    @if [ "{{component}}" = "*" ]; then \
        awk '{print $2}' checksums.txt | while read path; do \
            id=$(basename "$path"); \
            id="${id%.wasm}"; \
            id="${id//_/-}"; \
            echo "Publishing $path at wavs-tests:$id@{{version}}"; \
            wkg publish "$path" --package="wavs-tests:$id@{{version}}"; \
        done \
    else \
        awk '{print $2}' checksums.txt | while read path; do \
            id=$(basename "$path"); \
            id="${id%.wasm}"; \
            id="${id//_/-}"; \
            if [[ "$id" == "{{component}}" ]]; then \
                echo "Publishing $path at wavs-tests:$id@{{version}}"; \
                wkg publish "$path" --package="wavs-tests:$id@{{version}}"; \
            fi; \
        done \
    fi