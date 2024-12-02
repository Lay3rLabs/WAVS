#!/usr/bin/env bash

# cd to the directory of this script so that this can be run from anywhere
root_path=$(
    cd "$(dirname "${BASH_SOURCE[0]}")"
    cd ..
    pwd -P
)

middleware_path="$root_path/lib/eigenlayer-middleware"

cd "$middleware_path"
forge build 