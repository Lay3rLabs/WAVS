## Local Interchain

A Cosmos-SDK and Ethereum based e2e dockerized testing environment. [github repo](https://github.com/strangelove-ventures/interchaintest)

## Install

```bash
git clone --depth 1 --branch v8.8.0 https://github.com/strangelove-ventures/interchaintest.git ./ict
(cd ict/local-interchain && make install)
rm -rf ./ict/
```


## Running
```bash
local-ic start eth
```

# Generate Anvil State

## Base Eigenlayer deployed

```bash
anvil --state-interval 5 --dump-state ./chains/state/eigenlayer-deployed-anvil-state.json

# run wavs
(cd ./packages/wavs; cargo run)

# Deploy core contracts
(cd ./packages/cli; cargo run deploy-all --wavs)

# Wait a few seconds, then close the anvil state.
```
