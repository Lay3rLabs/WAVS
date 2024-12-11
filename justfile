SUDO := if `groups | grep docker` != "" { "" } else { "sudo" }
TAG := env_var_or_default("TAG", "")

help:
  @just --list

# builds wavs:latest
docker-build:
    {{SUDO}} docker build . -t ghcr.io/lay3rlabs/wavs:latest

# push wavs:latest to ghcr with optional TAG environment variable
docker-push:
    {{SUDO}} docker push ghcr.io/lay3rlabs/wavs:latest
    if [ "{{TAG}}" != "" ]; then \
        {{SUDO}} docker tag ghcr.io/lay3rlabs/wavs:latest ghcr.io/lay3rlabs/wavs:{{TAG}}; \
        {{SUDO}} docker push ghcr.io/lay3rlabs/wavs:{{TAG}}; \
    fi

# run wavs:latest
docker-run:
    {{SUDO}} docker run --rm ghcr.io/lay3rlabs/wavs:latest

# stop the running wavs container
docker-stop:
    #!/usr/bin/env bash
    DOCKER_ID=`docker ps | grep wavs | awk '{print $1}'`
    if [ "$DOCKER_ID" != "" ]; then \
        {{SUDO}} docker kill $DOCKER_ID; \
        echo "Stopped container $DOCKER_ID"; \
    else \
        echo "No container running"; \
    fi

# compile all WASI components, places the output in components dir
wasi-build:
    #!/usr/bin/env bash
    OUTDIR="./components"

    rm -rf examples/target/wasm32-wasip1/release/*.wasm "$OUTDIR"
    mkdir -p "$OUTDIR"

    BASEDIR=$(pwd)
    for C in examples/*/Cargo.toml; do
    DIR=$(dirname "$C")
    echo "Building WASI component in $DIR"
    (
        cd "$DIR";
        cargo component build --release
        cargo fmt
    )
    done

    cp examples/target/wasm32-wasip1/release/*.wasm "$OUTDIR"

    ls -l "$OUTDIR"
    cd "$OUTDIR"
    sha256sum -- *.wasm | tee checksums.txt

# compile solidity contracts and copy the ABI to contracts/abi
solidity-build:
    #!/usr/bin/env bash
    root_path=$(pwd)
    out="$root_path/contracts/abi"

    cd "$root_path"
    forge build
    cp -R out/* $out

    middleware_path="$root_path/lib/eigenlayer-middleware"
    cd "$middleware_path"
    forge build
    cp -r $middleware_path/out/* $out

    eigenlayer_contracts="$middleware_path/lib/eigenlayer-contracts"
    cd "$eigenlayer_contracts"
    forge build
    cp -r $eigenlayer_contracts/out/* $out


# on-chain integration test
test-wavs-e2e-ethereum:
    cargo test -p wavs --features e2e_tests_ethereum e2e_tests

update-submodules:
    git submodule update --init --recursive

lint:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings
