#!/usr/bin/env bash

# cd to the directory of this script so that this can be run from anywhere
parent_path=$(
    cd "$(dirname "${BASH_SOURCE[0]}")"
    pwd -P
)
cd "$parent_path"

cd ../

# Use .env file
source .env
export PRIVATE_KEY=$DEPLOYER_PRIVATE_KEY

forge script contracts/HelloWorldDeployer.s.sol --rpc-url $RPC_URL --broadcast
