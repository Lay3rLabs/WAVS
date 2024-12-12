# Quickstart

This guide will walk you through running [EigenLayer's Hello World AVS](https://docs.eigenlayer.xyz/developers/quickstart) example as a service using WAVS locally. 

## Setup

1. [Install Rust](https://www.rust-lang.org/tools/install). 
2. Install [Anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil), a local Ethereum development node, by running the following:

```bash
cargo install --git https://github.com/foundry-rs/foundry anvil --locked --force
```

3. Clone the WAVS repo:

```
git clone https://github.com/Lay3rLabs/WAVS.git
```

## Start Anvil

Open a new terminal and run the following command to start Anvil: 

```bash
Anvil
```

Your local chain will start running. Keep this terminal open. 

## Start WAVS

1. Open a second terminal in the `packages/wavs` directory of the WAVS repo. 

2. Copy the example `.env` file to the root of the directory.

```bash
cp .env.example .env
```

3. Create an empty directory titled `wavs-data` anywhere on your machine. Open the `.env` from the previous step, uncomment the first line, and specify the path for your `/wavs-data` directory. 

Example: 

```
WAVS_DATA=/path-to-your-folder/wavs-data
WAVS_SUBMISSION_MNEMONIC="test test test test test test test test test test test junk"
WAVS_LOG_LEVEL="info, wavs[{subsys=TriggerManager}]=debug"
```

4. Start WAVS by running the following command:

```bash
cargo run
```

WAVS should be running. Keep this terminal open. 

## Deploy and run the `hello-world` AVS

1. Open a third terminal in the `packages/cli` directory of the WAVS repo. 

2. Copy the example `.env` file to the root of the `/cli` directory.

```bash
cp .env.example .env
```

3. Run the following command to deploy the contracts and run the service: 

```bash
cargo run kitchen-sink --wavs
```

This `kitchen-sink` command will go through all the steps of:

1. Deploying core Eigenlayer contracts
2. Registering as an Eigenlayer operator
3. Deploying hello-world AVS contracts
4. Registering as a hello-world AVS operator
5. Creating a service on WAVS
6. Submitting a task to the hello-world on-chain contract
7. Waiting for WAVS to run the service and submit the result back on-chain

## Commands

Other commands are available to fine-tune services or run specific steps. Executing `cargo run -- --help` from within the `packages/cli` directory will give more info on each command

## Local vs. Testnet/Mainnet/etc.

The default `ws-endpoint` and `http-endpoint` for the CLI points to the local `Anvil` instance, which is the default `local` chain in WAVS. 

For other remote chains, make sure to change these endpoints accordingly, as well as the appropriate environment variables. 