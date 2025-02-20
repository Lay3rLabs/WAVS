#!/bin/bash

# Local Deployment assumes testnet strategies, for documentation on strategies on different chains see:
# https://github.com/layr-labs/eigenlayer-contracts In the README.md
STRATEGY_ADDRESSES='[
  "0x05037a81bd7b4c9e0f7b430f1f2a22c31a2fd943",
  "0x31b6f59e1627cefc9fa174ad03859fc337666af7",
  "0x3a8fbdf9e77dfc25d09741f51d3e181b25d0c4e0",
  "0x46281e3b7fdcacdba44cadf069a94a588fd4c6ef",
  "0x70eb4d3c164a6b4a5f908d4fbb5a9caffb66bab6",
  "0x7673a47463f80c6a3553db9e54c8cdcd5313d0ac",
  "0x78dbcbef8ff94ec7f631c23d38d197744a323868",
  "0x7d704507b76571a51d9cae8addabbfd0ba0e63d3",
  "0x80528d6e9a2babfc766965e0e26d5ab08d9cfaf9",
  "0x9281ff96637710cd9a5cacce9c6fad8c9f54631c",
  "0xaccc5a86732be85b5012e8614af237801636f8e5",
  "0xad76d205564f955a9c18103c4422d1cd94016899"
]'
# Build the `strategies` array as a Solidity-compatible input
# This is a workaround to get to a valid BPS value, in production strategies need to be weighed and maintained by an oracle 
declare -g combined_strategies=""
first_strategy=true

for strategy in $(echo "$STRATEGY_ADDRESSES" | jq -r '.[]'); do
    if [ "$first_strategy" = true ]; then
        combined_strategies+="(${strategy},837),"
        first_strategy=false
    else
        combined_strategies+="(${strategy},833),"
    fi
done
combined_strategies=${combined_strategies%,}

# prevents error where local run fails in rust script if you dont comment out TESTNET_RPC_URL in the env
if [ "$DEPLOY_ENV" = "LOCAL" ]; then
    unset TESTNET_RPC_URL
fi

wait_for_ethereum() {
    echo "Waiting for Ethereum node to be ready..."
    while ! curl -s -X POST -H "Content-Type: application/json" \
                 --data '{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}' \
                 "$LOCAL_ETHEREUM_RPC_URL" > /dev/null
    do
        sleep 1
    done
    echo "Ethereum node is ready!"
}

impersonate_account() {
    local account="$1"
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        return 0
    fi
    cast rpc anvil_impersonateAccount $account -r $LOCAL_ETHEREUM_RPC_URL > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        handle_error "Failed to impersonate account $account"
    fi
    cast rpc anvil_setBalance $account 0x10000000000000000000 -r $LOCAL_ETHEREUM_RPC_URL > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        handle_error "Failed to set balance for account $account"
    fi
}

handle_error() {
    local message="$1"
    echo "Error: $message"
    exit 1
}

check_env_var() {
    local var_name="$1"
    local var_value="$2"
    if [ -z "$var_value" ]; then
        handle_error "$var_name is not set in the environment variables"
    fi
}

execute_transaction() {
    local description="$1"
    local command="$2"

    eval "$command"

    if [ $? -eq 0 ]; then
        echo "Successfully $description"
    else
        handle_error "Failed to $description"
    fi
}

stop_impersonating() {
    local account="$1"
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        return 0
    fi
    cast rpc anvil_stopImpersonatingAccount "$account" -r "$LOCAL_ETHEREUM_RPC_URL" > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        echo "Error: Failed to stop impersonating account $account"
        exit 1
    fi
}

setup_operator() {
    local index=$1
    local layerServiceManagerAddress=$2
    if [ "$QUICK_MODE" = "ON" ] && [ "$index" -ne 1 ]; then
        echo "QUICK_MODE is ON - skipping operator setup for operator $index"
        return 0
    fi
    STRATEGY_MANAGER_ADDRESS=$(jq -r '.addresses.strategyManager' contracts/deployments/core/$CHAIN_ID.json)

    DELEGATION_MANAGER_ADDRESS=$(jq -r '.addresses.delegation' contracts/deployments/core/$CHAIN_ID.json)
    token_address=$(jq -r '.addresses.token' contracts/deployments/layer-middleware/$CHAIN_ID.json)
    strategy_address=$(jq -r '.addresses.strategy' contracts/deployments/layer-middleware/$CHAIN_ID.json)
    local mnemonic=$(cast wallet nm --json | jq -r '.mnemonic')
    local private_key=$(cast wallet pk "$mnemonic")
    local public_key=$(cast wallet address $private_key)
    
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        cast s "$public_key" --value 50000000000000000 --private-key "$FUNDED_KEY" -r "$LOCAL_ETHEREUM_RPC_URL" > /dev/null 2>&1
        if [ $? -ne 0 ]; then
            echo "Error: Failed to give operator $index balance"
            exit 1
        fi
        MINT_FUNCTION="submit(address _referral)"
        cast send "$LST_CONTRACT_ADDRESS" "$MINT_FUNCTION" "$public_key" "0x0000000000000000000000000000000000000000" \
            --private-key "$private_key" \
            --value 10000000000000000 \
            --rpc-url "$LOCAL_ETHEREUM_RPC_URL" > /dev/null 2>&1
        if [ $? -ne 0 ]; then
            echo "Error: Failed to mint LST for $ADDRESS"
            exit 1
        fi
        cast send "$LST_CONTRACT_ADDRESS" "approve(address,uint256)" \
            "$STRATEGY_MANAGER_ADDRESS" 10000000000000000 \
            --private-key "$private_key" \
            --rpc-url "$LOCAL_ETHEREUM_RPC_URL" > /dev/null 2>&1
        if [ $? -ne 0 ]; then
            echo "Error: Failed to approve LST for $STRATEGY_MANAGER_ADDRESS"
            exit 1
        fi
        cast send "$STRATEGY_MANAGER_ADDRESS" "depositIntoStrategy(address,address,uint256)" \
            "$LST_STRATEGY_ADDRESS" "$LST_CONTRACT_ADDRESS" 10000000000000000 \
            --private-key "$private_key" \
            --rpc-url "$LOCAL_ETHEREUM_RPC_URL" > /dev/null 2>&1
        if [ $? -ne 0 ]; then
            echo "Error: Failed to deposit into strategy for $LST_STRATEGY_ADDRESS"
            exit 1
        fi
    else
        cast rpc anvil_setBalance $public_key 0x10000000000000000000 -r $LOCAL_ETHEREUM_RPC_URL > /dev/null 2>&1
    fi
    if [ $? -ne 0 ]; then
        echo "Failed to set balance for operator $index"
        exit 1
    fi

    cast send "$DELEGATION_MANAGER_ADDRESS" \
        "registerAsOperator(address,uint32,string)" \
        "$public_key" 0 "foo.bar" \
        --private-key "$private_key" \
        --rpc-url "$LOCAL_ETHEREUM_RPC_URL" > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        echo "Error: Failed to register as operator for $DELEGATION_MANAGER_ADDRESS"
        exit 1
    fi

    allocationManager=$(cast call "$layerServiceManagerAddress" "allocationManager()" --rpc-url "$LOCAL_ETHEREUM_RPC_URL" | cast parse-bytes32-address)
    cast s "$allocationManager" \
        "registerForOperatorSets(address,(address,uint32[],bytes))" \
        "$public_key" \
        "($layerServiceManagerAddress,[1,2],0x1234)" \
        --private-key $private_key \
        --rpc-url $LOCAL_ETHEREUM_RPC_URL > /dev/null 2>&1
    if [ $? -eq 0 ]; then
        echo "Successfully registered operator $public_key to operator sets [1,2]"
    else
        echo "Error: Failed to register operator $public_key to operator sets"
        exit 1
    fi

    PRIVATE_KEY=$private_key
    cargo run --bin register_layer_operator > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        echo "Error: Failed to register operator $public_key to operator sets"
        exit 1
    fi
    echo "PRIVATE_KEY_${index}=$private_key" > ~/.nodes/operator$index
    echo "MNEMONIC_${index}=$mnemonic" > ~/.nodes/operator_mnemonic$index
}

create_operator_set() {
    local set_id="$1"
    local owner="$2"
    local layerServiceManagerAddress="$3"

    impersonate_account "$owner"
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        execute_transaction "created operator set $set_id with 1 strategy" \
          "cast s '$layerServiceManagerAddress' \
             'createOperatorSets((uint32,address[])[])' \
             '[($set_id,[$LST_STRATEGY_ADDRESS])]' \
             --private-key '$deployer_private_key' \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    else
        execute_transaction "created operator set $set_id with 1 strategy" \
          "cast s '$layerServiceManagerAddress' \
             'createOperatorSets((uint32,address[])[])' \
             '[($set_id,[$LST_STRATEGY_ADDRESS])]' \
             --from '$owner' \
             --unlocked \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    fi
    stop_impersonating "$owner"
}

update_avs_registrar() {
    local owner="$1"
    local layerServiceManagerAddress="$2"
    local avsRegistrarAddress="$3"

    impersonate_account "$owner"
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        execute_transaction "set up the AVSRegistrar through LayerServiceManager" \
          "cast s '$layerServiceManagerAddress' \
             'setAVSRegistrar(address)' \
             '$avsRegistrarAddress' \
             --private-key '$deployer_private_key' \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    else
        execute_transaction "set up the AVSRegistrar through LayerServiceManager" \
          "cast s '$layerServiceManagerAddress' \
             'setAVSRegistrar(address)' \
             '$avsRegistrarAddress' \
             --from '$owner' \
             --unlocked \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    fi
    stop_impersonating "$owner"
}

update_metadata_url() {
  if [ "$QUICK_MODE" = "ON" ]; then
      echo "QUICK_MODE is ON - skipping update metadata url"
      return 0
  fi
  if [ "$DEPLOY_ENV" = "TESTNET" ]; then
    if [ ! -f "$HOME/.nodes/avs_deploy.json" ]; then
        echo "Error: $HOME/.nodes/avs_deploy.json does not exist"
        exit 1
    fi
  fi

  serviceManagerAddress=$(jq -r '.addresses.layerServiceManager' "$HOME/.nodes/avs_deploy.json")
  if [ -z "$serviceManagerAddress" ] || [ "$serviceManagerAddress" = "null" ]; then
      echo "Error: Failed to read layerServiceManager from $HOME/.nodes/avs_deploy.json"
      exit 1
  fi

  metadataURI=$(jq -r '.metaDataURI' "$HOME/.nodes/avs_deploy.json")
  if [ -z "$metadataURI" ] || [ "$metadataURI" = "null" ]; then
      echo "Error: Failed to read metaDataURI from $HOME/.nodes/avs_deploy.json"
      exit 1
  fi

  owner=$(cast call "$serviceManagerAddress" "owner()" --rpc-url "$LOCAL_ETHEREUM_RPC_URL" | cast parse-bytes32-address)

  impersonate_account "$owner"
  if [ "$DEPLOY_ENV" = "TESTNET" ]; then
      execute_transaction "updated AVS metadata URI" \
        "cast s '$serviceManagerAddress' 'updateAVSMetadataURI(string)' \
         '$metadataURI' \
         --private-key '$deployer_private_key' \
         --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
  else
      execute_transaction "updated AVS metadata URI" \
        "cast s '$serviceManagerAddress' 'updateAVSMetadataURI(string)' \
         '$metadataURI' \
         --from '$owner' \
         --unlocked \
         --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
  fi
  stop_impersonating "$owner"

}

update_quorum_config() {
    local owner="$1"
    local stakeRegistryAddress="$2"
    if [ "$QUICK_MODE" = "ON" ]; then
        echo "QUICK_MODE is ON - skipping update quorum config"
        return 0
    fi
    impersonate_account "$owner"
    
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        execute_transaction "update quorum config" \
          "cast s '$stakeRegistryAddress' \
             'updateQuorumConfig(((address,uint96)[]),address[])' \
             '([$combined_strategies])' \
             '[]' \
             --private-key '$deployer_private_key' \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    else
        execute_transaction "update quorum config" \
          "cast s '$stakeRegistryAddress' \
             'updateQuorumConfig(((address,uint96)[]),address[])' \
             '([$combined_strategies])' \
             '[]' \
             --from '$owner' \
             --unlocked \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    fi
    if [ $? -eq 0 ]; then
    stop_impersonating "$owner"
    else
        echo "Error: Failed to update quorum config."
        exit 1
    fi
}

setup_mock_token_and_rewards() {
    local owner="$1"
    local layerServiceManagerAddress="$2"
    if [ "$QUICK_MODE" = "ON" ]; then
        echo "QUICK_MODE is ON - skipping setup_mock_token_and_rewards"
        return 0
    fi
    MOCK_TOKEN_SUPPLY=$(cast to-wei 100)
    cd contracts && execute_transaction "deployed mock rewards token" "forge script DeployMockTokenScript \
        --sig 'run(address,uint256)' \
        '$owner' \
        '$MOCK_TOKEN_SUPPLY' \
        --rpc-url $LOCAL_ETHEREUM_RPC_URL \
        --private-key '$deployer_private_key' \
        --broadcast > /dev/null 2>&1"

    tokenAddress=$(cat deployments/layer-middleware/mockToken$CHAIN_ID.json | jq -r '.MockToken')

    impersonate_account "$owner"
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        execute_transaction "approved layerServiceManager for mock token transfers" \
          "cast s '$tokenAddress' \
             'approve(address,uint256)' \
             '$layerServiceManagerAddress' \
             '$MOCK_TOKEN_SUPPLY' \
             --private-key '$deployer_private_key' \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    else
        execute_transaction "approved layerServiceManager for mock token transfers" \
          "cast s '$tokenAddress' \
             'approve(address,uint256)' \
             '$layerServiceManagerAddress' \
             '$MOCK_TOKEN_SUPPLY' \
             --from '$owner' \
             --unlocked \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    fi
    stop_impersonating "$owner"

    operator_addresses=(
         $(cast wallet address $(grep '^PRIVATE_KEY_[0-9]=' ~/.nodes/operator1 | cut -d'=' -f2))
         $(cast wallet address $(grep '^PRIVATE_KEY_[0-9]=' ~/.nodes/operator2 | cut -d'=' -f2))
         $(cast wallet address $(grep '^PRIVATE_KEY_[0-9]=' ~/.nodes/operator3 | cut -d'=' -f2))
     )

    sorted_addresses=($(
      for addr in "${operator_addresses[@]}"; do
        lower_addr=$(echo "$addr" | tr '[:upper:]' '[:lower:]')
        echo "$lower_addr"
      done | sort
    ))

    operator_rewards=""
    for i in "${!sorted_addresses[@]}"; do
        address="${sorted_addresses[$i]}"
        reward=$(cast to-wei $((i + 1)))
        operator_rewards+="(${address},${reward}),"
    done
    operator_rewards=${operator_rewards%,}

    impersonate_account "$owner"
    twoWeeksAgo=$(date -d "2 weeks ago" +%s)
    twoWeeksAgoRounded=$(( twoWeeksAgo / 604800 * 604800 ))
    if [ "$DEPLOY_ENV" = "TESTNET" ]; then
        execute_transaction "created AVS Directed Rewards Submission" \
          "cast s '$layerServiceManagerAddress' \
             'createOperatorDirectedAVSRewardsSubmission(((address,uint96)[],address,(address,uint256)[],uint32,uint32,string)[])' \
             '[([$combined_strategies],$tokenAddress,[$operator_rewards],$twoWeeksAgoRounded,604800,\"mock description\")]' \
             --private-key '$deployer_private_key' \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    else
        execute_transaction "created AVS Directed Rewards Submission" \
          "cast s '$layerServiceManagerAddress' \
             'createOperatorDirectedAVSRewardsSubmission(((address,uint96)[],address,(address,uint256)[],uint32,uint32,string)[])' \
             '[([$combined_strategies],$tokenAddress,[$operator_rewards],$twoWeeksAgoRounded,604800,\"mock description\")]' \
             --from '$owner' \
             --unlocked \
             --rpc-url '$LOCAL_ETHEREUM_RPC_URL' > /dev/null 2>&1"
    fi
    stop_impersonating "$owner"

    cd ..
}

deploy_consumer_contract() {
    cd ../../avs-ecdsa-sol-sdk
    offchainMessageConsumerAddress=$(
        forge create --json \
          -r "$LOCAL_ETHEREUM_RPC_URL" \
          --private-key "$deployer_private_key" \
          --broadcast src/example-contracts/OffchainMessageConsumer.sol:OffchainMessageConsumer \
          --constructor-args "$stakeRegistryAddress" \
        | jq -r '.deployedTo'
    )
    if [ -z "$offchainMessageConsumerAddress" ] || [ "$offchainMessageConsumerAddress" = "null" ]; then
        echo "Error: Failed to read offchainMessageConsumerAddress from $HOME/.nodes/avs_deploy.json"
        exit 1
    fi

    echo "OffchainMessageConsumer for e2e testing deployed with address: $offchainMessageConsumerAddress"
    cd ../layer-middleware
    jq --arg addr "$offchainMessageConsumerAddress" \
       '.addresses.offchainMessageConsumer = $addr' \
       contracts/deployments/layer-middleware/$CHAIN_ID.json \
       > temp.json && mv temp.json contracts/deployments/layer-middleware/$CHAIN_ID.json

    cp contracts/deployments/layer-middleware/$CHAIN_ID.json ~/.nodes/avs_deploy.json
}

#############################################
###### Start of script execution ############
#############################################

deployer_private_key=$(cast wallet new --json | jq -r '.[0].private_key')
deployer_public_key=$(cast wallet address "$deployer_private_key")
echo "PRIVATE_KEY=$deployer_private_key" >> contracts/.env
source contracts/.env

if [ "$DEPLOY_ENV" = "TESTNET" ]; then
    LOCAL_ETHEREUM_RPC_URL="$TESTNET_RPC_URL"
    if [ -z "$LOCAL_ETHEREUM_RPC_URL" ]; then
        echo "Error: TESTNET_RPC_URL environment variable is not set"
        exit 1
    fi
    if [ -z "$LST_CONTRACT_ADDRESS" ]; then
        echo "Error: LST_CONTRACT_ADDRESS is not set in the environment variables."
        exit 1
    fi
    if [ -z "$LST_STRATEGY_ADDRESS" ]; then
        echo "Error: LST_STRATEGY_ADDRESS is not set in the environment variables."
        exit 1
    fi
    if [ -n "$FUNDED_KEY" ] && [ -z "$FUNDED_KEY" ]; then
        echo "Error: FUNDED_KEY environment variable is set but empty"
        exit 1
    fi
    cast s "$deployer_public_key" --value 100000000000000000 \
    --private-key "$FUNDED_KEY" \
    -r "$LOCAL_ETHEREUM_RPC_URL" > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        echo "Error: Failed to set balance for deployer"
        exit 1
    fi
else
    LOCAL_ETHEREUM_RPC_URL=http://ethereum:8545
    wait_for_ethereum
    cast rpc anvil_setBalance $deployer_public_key 0x10000000000000000000 -r $LOCAL_ETHEREUM_RPC_URL > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        echo "Error: Failed to set balance for deployer"
        exit 1
    fi
fi

echo "Deployer address: $deployer_public_key configured for $DEPLOY_ENV environment"

cd contracts && forge script script/LayerMiddlewareDeployer.s.sol --rpc-url $LOCAL_ETHEREUM_RPC_URL --broadcast > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "Error: Failed to run middleware deployment script"
    exit 1
fi

stakeRegistryAddress=$(cat deployments/layer-middleware/$CHAIN_ID.json | jq -r '.addresses.stakeRegistry')
layerServiceManagerAddress=$(cat deployments/layer-middleware/$CHAIN_ID.json | jq -r '.addresses.layerServiceManager')
avsRegistrarAddress=$(cat deployments/layer-middleware/$CHAIN_ID.json | jq -r '.addresses.avsRegistrar')
[ -z "$stakeRegistryAddress" -o -z "$layerServiceManagerAddress" -o -z "$avsRegistrarAddress" ] && { echo "Error: One or more required addresses (stakeRegistryAddress, layerServiceManagerAddress, avsRegistrarAddress) are null or empty"; exit 1; }
echo "Middleware contracts deployed with addresses: LayerServiceManager: $layerServiceManagerAddress, StakeRegistry: $stakeRegistryAddress, AVSRegistrar: $avsRegistrarAddress"

owner=$(cast call "$stakeRegistryAddress" "owner()" --rpc-url "$LOCAL_ETHEREUM_RPC_URL" | cast parse-bytes32-address)

# This function is used to deploy the consumer contract for e2e testing via signature validation
deploy_consumer_contract

# This function is used to update the quorum config for the stake registry, defining the strategies and their BPS weights
update_quorum_config "$owner" "$stakeRegistryAddress"

# This function is used to update the AVS registrar for the stake registry, allowing injection of business logic to AVS registration
update_avs_registrar "$owner" "$layerServiceManagerAddress" "$avsRegistrarAddress"

# This function is used to create the operator sets for the stake registry, allowing meta-avs functionality or otherwise discerneable operator sets
for i in {1..2}; do
    create_operator_set "$i" "$owner" "$layerServiceManagerAddress"
done

# This function is used to register the operators to eigenlayer and the avs
for i in {1..3}; do
    setup_operator "$i" "$layerServiceManagerAddress"
done

# This function is used to setup the mock token and rewards for the stake registry, allowing the AVS to submit rewards to the operators
setup_mock_token_and_rewards "$owner" "$layerServiceManagerAddress"

# This function is used to update the metadata URL for the stake registry, allowing to be indexed by the Eigenlayer frontend
update_metadata_url

# This function is used to validate the signature of operators on the consumer contract, allowing for e2e signature validation testing
cargo run --bin validate_signature 3

# This function is used to keep the container running indefinitely, in order to allow re-running the script without having to restart the container
while true; do
    sleep 1
done
