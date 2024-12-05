#!/usr/bin/env bash

# cd to the directory of this script so that this can be run from anywhere
root_path=$(
    cd "$(dirname "${BASH_SOURCE[0]}")"
    cd ..
    pwd -P
)
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
