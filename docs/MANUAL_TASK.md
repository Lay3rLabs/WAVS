## Run a task manually

This guide shows you how to run WAVS locally and manually add a task manually using the hello-world AVS example.

**Note**: Before starting this guide, you'll need to follow the steps in the [Quickstart guide](./QUICKSTART.md) first. Make sure your `/wavs-data` directory is empty before starting this guide.

## Start Anvil

Open a new terminal and run the following command to start Anvil:

```bash
anvil
```

Your local chain will start running. Keep this terminal open.

## Start WAVS


1. Open a second terminal in the `packages/wavs` directory of the WAVS repo.

Note: Your `.env` file should already be present from when you ran the [Quickstart guide](./QUICKSTART.md).

2. Start WAVS by running the following command:

```bash
cargo run
```

WAVS should be running. Keep this terminal open.

## Deploy contracts and add a task

1. Open a third terminal in the `packages/cli` directory of the WAVS repo.

**Note**: Your `.env` file should already be present from when you ran the [Quickstart guide](./QUICKSTART.md).


2. Run the following command to deploy the necessary service contracts.

```
cargo run deploy-all --wavs
```

This command deploys all of the necessary contracts for the hello-world AVS without adding a task.

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


4. Copy the address associated with the `CLI_EIGEN_SERVICE_MANAGER` address and use it in the following command:

```
cargo run add-task --contract-address="<service-manager-address>" --wavs
```

This will add a task to be run. If the task has been run successfully, you will see a task response hash in your terminal.

If you submit a task without WAVS running, you'll notice that the task will time out and no result will be submitted onchain.
