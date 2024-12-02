#!/usr/bin/env bash

# cd to the directory of this script so that this can be run from anywhere
root_path=$(
    cd "$(dirname "${BASH_SOURCE[0]}")"
    cd ..
    pwd -P
)

middleware_path="$root_path/lib/eigenlayer-middleware"
middleware_out_path="$middleware_path/out"
middleware_dest_abi_path="$root_path/contracts/abi/eigenlayer-middleware"

cd "$middleware_path"
forge build 

cd "$root_path"
mkdir -p "$middleware_dest_abi_path"

cp "$middleware_out_path/RegistryCoordinator.sol/RegistryCoordinator.json" "$middleware_dest_abi_path/RegistryCoordinator.json"
cp "$middleware_out_path/OperatorStateRetriever.sol/OperatorStateRetriever.json" "$middleware_dest_abi_path/OperatorStateRetriever.json"
cp "$middleware_out_path/StakeRegistry.sol/StakeRegistry.json" "$middleware_dest_abi_path/StakeRegistry.json"
cp "$middleware_out_path/BLSApkRegistry.sol/BLSApkRegistry.json" "$middleware_dest_abi_path/BLSApkRegistry.json"
cp "$middleware_out_path/IBLSSignatureChecker.sol/IBLSSignatureChecker.json" "$middleware_dest_abi_path/IBLSSignatureChecker.json"
cp "$middleware_out_path/ServiceManagerBase.sol/ServiceManagerBase.json" "$middleware_dest_abi_path/ServiceManagerBase.json"
cp "$middleware_out_path/DelegationManager.sol/DelegationManager.json" "$middleware_dest_abi_path/DelegationManager.json"
cp "$middleware_out_path/ISlasher.sol/ISlasher.json" "$middleware_dest_abi_path/ISlasher.json"
cp "$middleware_out_path/StrategyManager.sol/StrategyManager.json" "$middleware_dest_abi_path/StrategyManager.json"
cp "$middleware_out_path/EigenPod.sol/EigenPod.json" "$middleware_dest_abi_path/EigenPod.json"
cp "$middleware_out_path/EigenPodManager.sol/EigenPodManager.json" "$middleware_dest_abi_path/EigenPodManager.json"
cp "$middleware_out_path/IStrategy.sol/IStrategy.json" "$middleware_dest_abi_path/IStrategy.json"
cp "$middleware_out_path/AVSDirectory.sol/AVSDirectory.json" "$middleware_dest_abi_path/AVSDirectory.json"
cp "$middleware_out_path/IRegistryCoordinator.sol/IRegistryCoordinator.json" "$middleware_dest_abi_path/IRegistryCoordinator.json"