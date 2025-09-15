SUDO := if `groups | grep -q docker > /dev/null 2>&1 && echo true || echo false` == "true" { "" } else { "sudo" }
TAG := env_var_or_default("TAG", "")
WASI_OUT_DIR := "./examples/build/components"
COSMWASM_OUT_DIR := "./examples/build/contracts"
REPO_ROOT := `git rev-parse --show-toplevel`
DOCKER_WAVS_ID := `docker ps | grep wavs | awk '{print $1}'`
ARCH := `uname -m`
COSMWASM_OPTIMIZER_VERSION := env_var_or_default("COSMWASM_OPTIMIZER_VERSION", "0.17.0")

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
    @cp "./wavs.toml" "{{HOME}}"
    @cp "./.env.example" "{{HOME}}/.env"
    @cargo install --path ./packages/wavs
    @cargo install --path ./packages/cli
    @cargo install --path ./packages/aggregator
    @echo "Add these variables to your system environment:"
    @echo ""
    @echo "export WAVS_HOME=\"{{HOME}}\""
    @echo "export WAVS_DATA=\"{{DATA}}/wavs\""
    @echo "export WAVS_CLI_HOME=\"{{HOME}}\""
    @echo "export WAVS_CLI_DATA=\"{{DATA}}/wavs-cli\""
    @echo "export WAVS_AGGREGATOR_HOME=\"{{HOME}}\""
    @echo "export WAVS_AGGREGATOR_DATA=\"{{DATA}}/wavs-aggregator\""
    @echo "export WAVS_DOTENV=\"{{HOME}}/.env\""

wasi-builder-build TAG="latest":
    IMAGE_TAG=ghcr.io/lay3rlabs/wavs-wasi-builder:{{TAG}} bash tools/wasi-builder/build-image.sh

wasi-builder-push TAG="latest":
    # Build and push a multi-arch manifest (amd64, arm64)
    export DOCKER_BUILDKIT=1; \
    if ! docker buildx inspect >/dev/null 2>&1; then docker buildx create --name wavs-bx --use >/dev/null; fi; \
    docker buildx build \
      --platform linux/amd64,linux/arm64 \
      -f tools/wasi-builder/Dockerfile \
      -t ghcr.io/lay3rlabs/wavs-wasi-builder:{{TAG}} \
      --push \
      tools/wasi-builder

wasi-build COMPONENT="*" TAG="latest":
    WASI_BUILDER_IMAGE=ghcr.io/lay3rlabs/wavs-wasi-builder:{{TAG}} bash tools/wasi-builder/build-components.sh '{{COMPONENT}}'

# compile solidity contracts (including examples) and copy the ABI to contracts/solidity/abi
# example ABI's will be copied to examples/contracts/solidity/abi
solidity-build CLEAN="":
    @if [ "{{CLEAN}}" = "clean" ]; then \
        rm -rf {{REPO_ROOT}}/out; \
        rm -rf {{REPO_ROOT}}/packages/types/src/contracts/solidity/abi; \
        rm -rf {{REPO_ROOT}}/examples/contracts/solidity/abi; \
    fi
    mkdir -p {{REPO_ROOT}}/out
    mkdir -p {{REPO_ROOT}}/packages/types/src/contracts/solidity/abi
    mkdir -p {{REPO_ROOT}}/examples/contracts/solidity/abi
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/out --contracts {{REPO_ROOT}}/contracts/solidity;
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/out --contracts {{REPO_ROOT}}/examples/contracts/solidity;
    # examples
    cp -r {{REPO_ROOT}}/out/SimpleTrigger.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/ISimpleTrigger.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/SimpleSubmit.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/ISimpleSubmit.sol {{REPO_ROOT}}/examples/contracts/solidity/abi/
    # wavs-types
    cp -r {{REPO_ROOT}}/out/IWavsServiceHandler.sol {{REPO_ROOT}}/packages/types/src/contracts/solidity/abi/
    cp -r {{REPO_ROOT}}/out/IWavsServiceManager.sol {{REPO_ROOT}}/packages/types/src/contracts/solidity/abi/

# compile cosmwasm example contracts
cosmwasm-build CONTRACT="*":
    rm -rf ./artifacts/*.wasm
    rm -rf {{COSMWASM_OUT_DIR}}
    mkdir -p {{COSMWASM_OUT_DIR}}

    @for C in examples/contracts/cosmwasm/{{CONTRACT}}/Cargo.toml; do \
        just cosmwasm-build-inner $(dirname $C); \
    done

    cp ./artifacts/*.wasm {{COSMWASM_OUT_DIR}}

cosmwasm-build-inner CONTRACT_PATH:
    @if [ "{{ARCH}}" = "arm64" ]; then \
        docker run --rm \
            -v "{{REPO_ROOT}}:/code" \
            --mount type=volume,source="layer_wavs_cache",target=/target \
            --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
            cosmwasm/optimizer-arm64:{{COSMWASM_OPTIMIZER_VERSION}} "{{CONTRACT_PATH}}"; \
    else \
        docker run --rm \
            -v "{{REPO_ROOT}}:/code" \
            --mount type=volume,source="layer_wavs_cache",target=/target \
            --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
            cosmwasm/optimizer:{{COSMWASM_OPTIMIZER_VERSION}} "{{CONTRACT_PATH}}"; \
    fi;
# on-chain integration test
test-wavs-e2e:
    RUST_LOG=debug,alloy_rpc=off,alloy_provider=off,wasmtime=off,cranelift=off,hyper_util=off cargo test -p layer-tests

update-submodules:
    git submodule update --init --recursive

lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

lint-fix:
    cargo fmt --all
    cargo fix --workspace --all-targets --allow-dirty --allow-staged
    cargo clippy --fix --workspace --all-targets --allow-dirty -- -D warnings
    cargo check --workspace --all-targets

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

start-dev:
    #!/bin/bash -eux
    just start-prometheus &
    just start-jaeger &
    just start-wavs-dev &
    trap 'kill $(jobs -pr)' EXIT
    wait

start-wavs-dev:
    #!/bin/bash -eu
    TEMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TEMP_DIR"' EXIT
    cd packages/wavs && \
    WAVS_DOTENV="$(PWD)/../../.env" WAVS_HOME="$(PWD)/../../" WAVS_DATA="$TEMP_DIR" \
    cargo run -- \
        --dev-endpoints-enabled=true \
        --disable-trigger-networking=true \
        --disable-submission-networking=true \
        --prometheus="http://127.0.0.1:9090" \
        --jaeger="http://127.0.0.1:4317"

start-jaeger:
    docker run --rm -p 4317:4317 -p 16686:16686 jaegertracing/jaeger:2.5.0

start-prometheus:
    docker run --rm -p 9090:9090 -v ./prometheus.yml:/etc/prometheus/prometheus.yml prom/prometheus --config.file=/etc/prometheus/prometheus.yml --web.enable-otlp-receiver

dev-tool *args:
    cd packages/dev-tool && RUST_LOG=info cargo run -- {{args}}

start-aggregator:
    cd packages/aggregator && cargo run

start-anvil:
    anvil

# e.g. `just cli-exec ./examples/build/components/echo_data.wasm "hello world"`
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
    rm -rf wit-definitions
    mkdir -p wit-definitions

    # Copy it over
    cp -r temp_clone/wavs-wasi/wit-definitions/* wit-definitions/

    # Fetch deps
    cd wit-definitions/operator && wkg wit fetch
    cd wit-definitions/aggregator && wkg wit fetch
    cd wit-definitions/types && wkg wit fetch

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
    mkdir -p examples/contracts/solidity/mocks

    # Copy just the interfaces
    cp temp_clone/wavs-middleware/contracts/src/eigenlayer/ecdsa/interfaces/*.sol contracts/solidity/interfaces/

    # and, for examples - interfaces and mocks
    cp temp_clone/wavs-middleware/contracts/src/eigenlayer/ecdsa/interfaces/*.sol examples/contracts/solidity/interfaces/
    cp temp_clone/wavs-middleware/contracts/src/eigenlayer/ecdsa/mocks/*.sol examples/contracts/solidity/mocks/

    # Clean up
    rm -rf temp_clone

wasi-publish version component="*" flags="":
	if [ "{{component}}" = "*" ]; then \
	    awk '{print $2}' checksums.txt | while read path; do \
	        id=$(basename "$path"); \
	        id="${id%.wasm}"; \
	        id=$(echo "$id" | sed 's/_/-/g'); \
	        echo "Publishing $path at wavs-tests:$id@{{version}}"; \
	        wkg publish "$path" --package="wavs-tests:$id@{{version}}" {{flags}}; \
	    done; \
	else \
	    awk '{print $2}' checksums.txt | while read path; do \
	        id=$(basename "$path"); \
	        id="${id%.wasm}"; \
	        id=$(echo "$id" | sed 's/_/-/g'); \
	        if [ "$id" = "{{component}}" ]; then \
	            echo "Publishing $path at wavs-tests:$id@{{version}}"; \
	            wkg publish "$path" --package="wavs-tests:$id@{{version}}" {{flags}}; \
	        fi; \
	    done; \
	fi
