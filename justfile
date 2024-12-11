SUDO := if `groups | grep docker` != "" { "" } else { "sudo" }
TAG := env_var_or_default("TAG", "")
WASI_OUT_DIR := "./components"
REPO_ROOT := `git rev-parse --show-toplevel`
DOCKER_WAVS_ID := `docker ps | grep wavs | awk '{print $1}'`

help:
  @just --list

# builds wavs:latest
docker-build:
    {{SUDO}} docker build . -t ghcr.io/lay3rlabs/wavs:latest

# push wavs:latest to ghcr with optional TAG environment variable
docker-push:
    {{SUDO}} docker push ghcr.io/lay3rlabs/wavs:latest
    @if [ "{{TAG}}" != "" ]; then \
        {{SUDO}} docker tag ghcr.io/lay3rlabs/wavs:latest ghcr.io/lay3rlabs/wavs:{{TAG}}; \
        {{SUDO}} docker push ghcr.io/lay3rlabs/wavs:{{TAG}}; \
    fi

# run wavs:latest
docker-run:
    {{SUDO}} docker run --rm ghcr.io/lay3rlabs/wavs:latest

# stop the running wavs container
docker-stop:
    @if [ "{{DOCKER_WAVS_ID}}" != "" ]; then \
        {{SUDO}} docker kill {{DOCKER_WAVS_ID}}; \
        echo "Stopped container {{DOCKER_WAVS_ID}}"; \
    else \
        echo "No container running"; \
    fi

# compile all WASI components, places the output in components dir
wasi-build:
    @rm -rf examples/target/wasm32-wasip1/release/*.wasm {{WASI_OUT_DIR}}
    @mkdir -p {{WASI_OUT_DIR}}

    @for C in examples/*/Cargo.toml; do \
        echo "Building WASI component in $(dirname $C)"; \
        `cd $(dirname $C); cargo component build --release; cargo fmt;`; \
    done

    @cp ./examples/target/wasm32-wasip1/release/*.wasm {{WASI_OUT_DIR}}
    @sha256sum -- {{WASI_OUT_DIR}}/*.wasm | tee checksums.txt

# compile solidity contracts and copy the ABI to contracts/abi
solidity-build:
    forge build --root {{REPO_ROOT}} --out {{REPO_ROOT}}/contracts/abi;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-middleware --out {{REPO_ROOT}}/contracts/abi;
    forge build --root {{REPO_ROOT}}/lib/eigenlayer-contracts --out {{REPO_ROOT}}/contracts/abi;

# on-chain integration test
test-wavs-e2e-ethereum:
    cargo test -p wavs --features e2e_tests_ethereum e2e_tests

update-submodules:
    git submodule update --init --recursive

lint:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings
