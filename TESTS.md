We have 3 levels of tests: unit, mock-integration, and e2e

Unit and mock-integration tests run with regular `cargo test`. e2e tests are more involved and are gated behind the `e2e_tests` and `e2e_tests_*` features.

The logging level is set via `RUST_LOG` env var, see [LOGGING.md](./LOGGING.md) for more info 

## Unit tests

These are generally written in the same file as the code they're testing

You can run them faster, skipping other tests, by running `cargo test --lib`

## Mock-integration

These are written in the `tests` folder and use mock structures to test both the whole flow (mock_e2e_*) and how different parts of the system connect.

You can run them with `cargo test`

## e2e (a.k.a. "on-chain")

This is where it gets complicated. These tests require that a real chain be running, with the core contracts deployed, and then it tests live functionality by creating a service, submitting a task, and waiting for it to complete on-chain.

### Ethereum e2e tests

There are various ways to get a real chain with the contracts deployed, the following is one workable approach.

1. Follow the steps in [EIGENLAYER.md](./EIGENLAYER.md)
2. Copy the wallet mnemonic to `WAVS_E2E_ETHEREUM_MNEMONIC` env var (probably to "test test test test test test test test test test test junk")
3. `cargo test --features e2e_tests,e2e_tests_ethereum e2e_tests`




### Layer e2e tests

There are various ways to get a real chain with the contracts deployed, the following is one workable approach.

First make sure you've cloned the [my-layer](https://github.com/Lay3rLabs/my-layer) and [avs-toolkit](https://github.com/Lay3rLabs/avs-toolkit) repos

_Tip!_: It may be helpful to have 3 different terminals open: for this repo, `my-layer`, and `avs-toolkit`, since we will be running commands in each of them separately.

_Tip!_: For setting env vars, all the tooling is `.env` file aware, from wherever the command is run, so you can use that to set env vars if you prefer. 

1. In this repo, build the services
    - `scripts/build_wasi.sh`
    - success is if you see a final output of `.wasm` files with their hash
2. Start localnode in `my-layer` repo
    - `localnode/run.sh`
    - more info here: https://github.com/Lay3rLabs/my-layer/blob/main/localnode/DEMO.md#start-localnode)
3. In `avs-toolkit`, under the `tools/cli` directory, setup a wallet
    - create the wallet: `cargo run -- --target=local wallet create`
    - set the mnemonic as `LOCAL_MNEMONIC` env var (needed for avs-toolkit to see it)
    - tap the faucet: `cargo run -- --target=local faucet tap`
    - more info here: https://github.com/Lay3rLabs/my-layer/blob/main/localnode/DEMO.md#set-up-wallet
4. Set this wallet mnemonic in the `WAVS_E2E_COSMOS_MNEMONIC` env var
5. In `avs-toolkit`, under the `tools/cli` directory, deploy the contracts
    - build the contracts: `(cd ../.. && ./scripts/optimizer.sh)`
        - success is if you see a final output of `.wasm` files with their hash
    - deploy the contracts: `cargo run -- --target=local deploy -m verifier-simple contracts --operators wasmatic`
    - take the LOCAL_TASK_QUEUE_ADDRESS value, and set it in the `WAVS_E2E_COSMOS_TASK_QUEUE_ADDRESS` env var
6. Kill the localnode's wasmatic instance to make sure we're hitting the test instance only: `docker stop localnode-wasmatic-1`
7. In this repo, run the tests: `cargo test --workspace --locked --features e2e_tests_cosmos_baseline`