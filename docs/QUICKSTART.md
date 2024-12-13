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

1. Open a second terminal in the `packages/wavs` directory of the WAVS repo.

2. Copy the example `.env` file to the root of the directory.

```bash
cp .env.example .env
```

3. Open the `.env` from the previous step, uncomment the first line, and specify the absolute file path for your WAVS data directory.

> NOTE: The '~' symbol is expanded into your home directory, like `/home/username`.

Example:

```
WAVS_DATA=~/wavs/data
WAVS_SUBMISSION_MNEMONIC="test test test test test test test test test test test junk"
WAVS_LOG_LEVEL="info, wavs[{subsys=TriggerManager}]=debug"
```

4. Start WAVS by running:

```bash
cargo run
```

WAVS is now running. Keep this terminal open.

## Deploy and run the `hello-world` AVS

1. Open a third terminal in the `packages/cli` directory of the WAVS repo.

2. Copy the example `.env` file to the root of the `/cli` directory.

```bash
cp .env.example .env
```

3. Deploy the contracts and run the service:

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
