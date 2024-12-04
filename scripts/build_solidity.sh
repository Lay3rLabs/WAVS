#!/usr/bin/env bash

# cd to the directory of this script so that this can be run from anywhere
root_path=$(
    cd "$(dirname "${BASH_SOURCE[0]}")"
    cd ..
    pwd -P
)

cd "$root_path"
forge build

cp -TR out/ contracts/abi
