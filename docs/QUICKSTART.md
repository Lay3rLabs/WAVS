# Quickstart

This guide walks you through running [EigenLayer's Hello World AVS](https://docs.eigenlayer.xyz/developers/quickstart) example as a service using WAVS locally.

## Setup

1. [Install Rust](https://www.rust-lang.org/tools/install).
2. Install [Anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil), a local Ethereum development node, by running:

```bash
cargo install --git https://github.com/foundry-rs/foundry anvil --locked --force
```

3. Clone the WAVS repo:

```bash
git clone https://github.com/Lay3rLabs/WAVS.git
```

## Start Anvil

Open a new terminal and run the following command to start Anvil:

```bash
anvil
```

Your local chain is now running. Keep this terminal open.

## Start WAVS

4. Open a second terminal in the `packages/wavs` directory of the WAVS repo.

5. Copy the example `.env` file to the root of the directory.

```bash
cp .env.example .env
```

6. Open the `.env` from the previous step, uncomment the first line, and specify the absolute file path for your WAVS data directory.

> NOTE: The '~' symbol is expanded into your home directory, like `/home/username`.

Example:

```
WAVS_DATA=~/wavs/data
WAVS_SUBMISSION_MNEMONIC="test test test test test test test test test test test junk"
WAVS_LOG_LEVEL="info, wavs[{subsys=TriggerManager}]=debug"
```

7. Start WAVS by running:

```bash
cargo run
```

WAVS is now running. Keep this terminal open.

## Deploy and run the `hello-world` AVS

8. Open a third terminal in the `packages/cli` directory of the WAVS repo.

9. Copy the example `.env` file to the root of the `/cli` directory.

```bash
cp .env.example .env
```

10. Run the following command to deploy all of the contracts for the hello-world AVS:

```
cargo run deploy-all --wavs
```

The output of your terminal should look like the following:

```
--- HELLO WORLD DIGEST ---
CLI_DIGEST_HELLO_WORLD="15e5e07adbb0be551772bc74fb182000f0efe7ea37a65622007042f47787d3b6"

--- HELLO WORLD SERVICE ID ---
0193b811ae997243a97ac48156b13296

--- CORE AVS CONTRACTS ---
CLI_EIGEN_CORE_PROXY_ADMIN="0x3347B4d90ebe72BeFb30444C9966B2B990aE9FcB"
CLI_EIGEN_CORE_DELEGATION_MANAGER="0x5bf5b11053e734690269C6B9D438F8C9d48F528A"
CLI_EIGEN_CORE_STRATEGY_MANAGER="0x457cCf29090fe5A24c19c1bc95F492168C0EaFdb"
CLI_EIGEN_CORE_POD_MANAGER="0x5fc748f1FEb28d7b76fa1c6B07D8ba2d5535177c"
CLI_EIGEN_CORE_POD_BEACON="0xF32D39ff9f6Aa7a7A64d7a4F00a54826Ef791a55"
CLI_EIGEN_CORE_PAUSER_REGISTRY="0x6F6f570F45833E249e27022648a26F4076F48f78"
CLI_EIGEN_CORE_STRATEGY_FACTORY="0x5FeaeBfB4439F3516c74939A9D04e95AFE82C4ae"
CLI_EIGEN_CORE_STRATEGY_BEACON="0xFD6F7A6a5c21A3f503EBaE7a473639974379c351"
CLI_EIGEN_CORE_AVS_DIRECTORY="0xab16A69A5a8c12C732e0DEFF4BE56A70bb64c926"
CLI_EIGEN_CORE_REWARDS_COORDINATOR="0xb9bEECD1A582768711dE1EE7B0A1d582D9d72a6C"

--- HELLO WORLD AVS CONTRACTS ---
CLI_EIGEN_SERVICE_PROXY_ADMIN="0xefc1aB2475ACb7E60499Efb171D173be19928a05"
CLI_EIGEN_SERVICE_MANAGER="0xB2b580ce436E6F77A5713D80887e14788Ef49c9A"
CLI_EIGEN_SERVICE_STAKE_REGISTRY="0xD49a0e9A4CD5979aE36840f542D2d7f02C4817Be"
CLI_EIGEN_SERVICE_STAKE_TOKEN="0x66F625B8c4c635af8b74ECe2d7eD0D58b4af3C3d"
```


11. Copy the address associated with `CLI_EIGEN_SERVICE_MANAGER` and use it in the following command:

```
cargo run add-task --contract-address="<service-manager-address>" --wavs
```

This adds a task to be run. If the task runs successfully, you will see a task response hash in your terminal.

If you submit a task without WAVS running, the task will time out, and no result will be submitted onchain.


## `kitchen-sync`

You can skip steps 10 and 11 above by using the `kitchen-sink` command:

```bash
cargo run kitchen-sink --wavs
```

This `kitchen-sink` command goes through all the steps of:

1. Deploying core Eigenlayer contracts
2. Registering as an Eigenlayer operator
3. Deploying hello-world AVS contracts
4. Registering as a hello-world AVS operator
5. Creating a service on WAVS
6. Submitting a task to the hello-world on-chain contract
7. Waiting for WAVS to run the service and submit the result back on-chain

## Commands

Other commands are available to fine-tune services or run specific steps. Executing `cargo run -- --help` in the `packages/cli` directory will give more info on each command

## Local vs. Testnet/Mainnet/etc.

The default `ws-endpoint` and `http-endpoint` for the CLI points to the local `Anvil` instance, which is the default `local` chain in WAVS.

For other remote chains, make sure to change these endpoints accordingly, as well as the appropriate environment variables.
