A service is ultimately composed of Workflows, which each contain a `Trigger`, `Component`, and `Submit`.

The `Trigger` and `Submit` are often Smart Contracts, and you're not limited whatsoever by following the WAVS examples.

## Prerequisites

To work with smart contracts in WAVS, you'll need to install Foundry (which includes Forge and Anvil):

```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

This installs `forge` for compiling contracts and `anvil` for running a local Ethereum node.

# Triggers

This is actually outside the scope of anything WAVS cares about — you can use any contract you want for your trigger, it doesn't need to follow any known interface or satisfy any custom message type. You may also use any tooling you wish to deploy it. The important thing is to pay attention to the event you want to use for the trigger. On EVM, this will be the event signature (a.k.a. "topic 0") and on Cosmos it will be your event type (i.e. the `event.ty` field).

When you create a service with a contract event trigger, you simply tell WAVS the contract address and event.

# Submit

The `submit` field in a workflow controls whether and how results are posted on-chain.

## Submit::None

Set `submit` to `None` when on-chain confirmation isn't needed. The operator component runs and may perform any side effect (e.g. posting to an external API), but nothing is signed or submitted to a blockchain. No service handler contract is required.

## Submit::Aggregator

When targeting a blockchain, the aggregator component decides which on-chain service handler contract to call and what data to submit — but the actual transaction (including gas fees) is submitted by the WAVS node. The contract must satisfy the appropriate interface for its chain.

### EVM

For EVM chains, your contract needs to satisfy the [IWavsServiceHandler interface](../contracts/solidity/interfaces/IWavsServiceHandler.sol).

It doesn't do very much — that's precisely the point. It's completely up to you for processing that data and handling it however you want. This is where you put all your business logic — no limits!

### Cosmos

For Cosmos chains, a CosmWasm contract plays the same role as the EVM service handler. The reference implementation lives at [Lay3rLabs/cw-middleware](https://github.com/Lay3rLabs/cw-middleware/tree/main/packages/contracts/mock) — use it as a starting point or replace it with your own contract. (A copy is pulled into `examples/contracts/cosmwasm/mock/` for local e2e tests only.)
