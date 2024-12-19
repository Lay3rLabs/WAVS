# Quickstart

This guide walks you through how to set up your environment, deploy an example service, and run a task.

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

1. Open the `.env` from the previous step and specify the absolute file path for your WAVS data directory.

> NOTE: The '~' symbol is expanded into your home directory, like `/home/username`.

Example:

```
WAVS_DATA="~/wavs/data"
WAVS_SUBMISSION_MNEMONIC="test test test test test test test test test test test junk"
WAVS_LOG_LEVEL="info, wavs[{subsys=TriggerManager}]=debug"
```

7. Start WAVS by running:

```bash
cargo run
```

WAVS is now running. Keep this terminal open.

## Deploy service contracts

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
--- ETH TRIGGER ECHO DIGEST ---
CLI_DIGEST_ETH_TRIGGER_ECHO="a8e29a6341054eb0b236ad6a15c613d3404e4831dbffa5c89cb593f291f3eae6"

--- ETH_TRIGGER_ECHO SERVICE ID ---
0193dc8e7a82785197c43aeba077f17f

--- CORE AVS CONTRACTS ---
CLI_EIGEN_CORE_PROXY_ADMIN="0x5bf5b11053e734690269C6B9D438F8C9d48F528A"
CLI_EIGEN_CORE_DELEGATION_MANAGER="0x3aAde2dCD2Df6a8cAc689EE797591b2913658659"
CLI_EIGEN_CORE_STRATEGY_MANAGER="0x38a024C0b412B9d1db8BC398140D00F5Af3093D4"
CLI_EIGEN_CORE_POD_MANAGER="0x2a810409872AfC346F9B5b26571Fd6eC42EA4849"
CLI_EIGEN_CORE_POD_BEACON="0x99dBE4AEa58E518C50a1c04aE9b48C9F6354612f"
CLI_EIGEN_CORE_PAUSER_REGISTRY="0xB0f05d25e41FbC2b52013099ED9616f1206Ae21B"
CLI_EIGEN_CORE_STRATEGY_FACTORY="0x19cEcCd6942ad38562Ee10bAfd44776ceB67e923"
CLI_EIGEN_CORE_STRATEGY_BEACON="0x5302E909d1e93e30F05B5D6Eea766363D14F9892"
CLI_EIGEN_CORE_AVS_DIRECTORY="0x1f10F3Ba7ACB61b2F50B9d6DdCf91a6f787C0E82"
CLI_EIGEN_CORE_REWARDS_COORDINATOR="0x40918Ba7f132E0aCba2CE4de4c4baF9BD2D7D849"

--- LAYER AVS CONTRACTS ---
CLI_EIGEN_SERVICE_PROXY_ADMIN="0xD49a0e9A4CD5979aE36840f542D2d7f02C4817Be"
CLI_EIGEN_SERVICE_MANAGER="0x66F625B8c4c635af8b74ECe2d7eD0D58b4af3C3d"
CLI_EIGEN_SERVICE_TRIGGER="0x5D42EBdBBa61412295D7b0302d6F50aC449Ddb4F"
CLI_EIGEN_SERVICE_STAKE_REGISTRY="0xc582Bc0317dbb0908203541971a358c44b1F3766"
CLI_EIGEN_SERVICE_STAKE_TOKEN="0x74Cf9087AD26D541930BaC724B7ab21bA8F00a27"
```


11.  Copy the values associated with `CLI_EIGEN_SERVICE_TRIGGER`, `CLI_EIGEN_SERVICE_MANAGER`, and `ETH_TRIGGER_ECHO SERVICE ID`, and use them in the following command:

```
cargo run add-task --trigger-addr <TRIGGER_ADDR> --service-manager-addr <SERVICE_MANAGER_ADDR> --service-id <SERVICE_ID>
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
3. Deploying example AVS contracts
4. Registering as an AVS operator
5. Creating a service on WAVS
6. Submitting a task to the on-chain example contract
7. Waiting for WAVS to run the service and submit the result back on-chain

## Commands

Other commands are available to fine-tune services or run specific steps. Executing `cargo run --help` will give more info on each command

## Local vs. Testnet/Mainnet/etc.

The default `ws-endpoint` and `http-endpoint` for the CLI points to the local `Anvil` instance, which is the default `local` chain in WAVS.

For other remote chains, make sure to change these endpoints accordingly, as well as the appropriate environment variables.
