# Make some WAVS

```bash
# start anvil, wavs, and the aggregator
just start-all

# deploy all of Eigenlayer's core contracts
just cli-deploy-core

# Grab deployment information
CONTRACTS=`cat ~/wavs/cli/deployments.json | jq -r .eigen_core.local`
export FOUNDRY_ANVIL_PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
export CLI_EIGEN_CORE_DELEGATION_MANAGER=`echo $CONTRACTS | jq -r .delegation_manager`
export CLI_EIGEN_CORE_REWARDS_COORDINATOR=`echo $CONTRACTS | jq -r .rewards_coordinator`
export CLI_EIGEN_CORE_AVS_DIRECTORY=`echo $CONTRACTS | jq -r .avs_directory`

# deploy smart contract(s)
forge script ./contracts/script/ServiceManager.s.sol --rpc-url http://127.0.0.1:8545 --broadcast

# this has to be uploaded first since the service manager depends on it
ECDSA_STAKE_REGISTRY_ADDRESS=`cat broadcast/ServiceManager.s.sol/31337/run-latest.json | jq -r .transactions[0].contractAddress`
SERVICE_MANAGER_ADDRESS=`cat broadcast/ServiceManager.s.sol/31337/run-latest.json | jq -r .transactions[1].contractAddress`

# deploy your component (all permissions by default)
# just cli-deploy-service ./components/eth_trigger_square.wasm

# override the services manager if we uploaded a different one in the past (i.e. forge script)
# useful to create other components not called 'LayerServiceManager.sol' (since DeployService hardcodes this)
(cd packages/cli && cargo run deploy-service --component "./components/eth_trigger_square.wasm" --service-manager ${SERVICE_MANAGER_ADDRESS} --ecdsa-stake-registry ${ECDSA_STAKE_REGISTRY_ADDRESS})

## Add a task
just cli-add-task 01942e7349df79e387dc208adbc7647d {\"x\":2}
```
