First, open up 3 different terminals:

1. (local-only, from anywhere) - to run Anvil, the local Ethereum chain emulator 
2. in `packages/wavs` to run wavs
3. in `packages/cli` to run the CLI

Next, make sure you set the required `env` vars. An easy way is copy the `.env.example` file in each directory to `.env` and edit from there

Finally, run things in this order:

1. (local-only) `anvil`
2. `cargo run` (in `packages/wavs`)
3. `cargo run kitchen-sink --wavs` (in `packages/cli`)

This kitchen-sink command will go through all the steps of:

1. Deploying core eigenlayer contracts
2. Deploying hello-world AVS contracts
3. Creating a service on WAVS
4. Submitting a task to the hello-world on-chain contract
5. Waiting for WAVS to do its thing and get the result back on-chain

Other commands are available to fine-tune this and run specific steps.

Executing `cargo run -- --help` from within the `packages/cli` directory will give more info.

### Local vs. Testnet/Mainnet/etc.

The default `ws-endpoint` and `http-endpoint` for the CLI is pointing to the local `Anvil` instance, as is the default `local` chain in WAVS

If hitting some other remote chain, make sure to change these accordingly, as well as the env vars. 

### Debugging

One common problem is that the wavs data directory is set to a place that requires superuser permissions or does not exist.

Simply uncomment the `WAVS_DATA` env var and set it to someplace reasonable.

