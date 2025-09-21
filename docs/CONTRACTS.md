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

This is actually outside the scope of anything WAVS cares about - you can use any contract you want for your trigger, it doesn't need to follow any known interface or satisfy any custom message type. You may also use any tooling you wish to deploy it. The important thing is to pay attention to the event you want to use for the trigger. On EVM, this will be the event signature (a.k.a. "topic 0") and on Cosmos it will be your event type (i.e. the `event.ty` field).

When you create a service with a contract event trigger, you simply tell WAVS the contract address and event.

# Submit

For the Submit target, when targetting Eigenlayer, your contract needs to satisfy the [IWavsServiceHandler interface](../contracts/solidity/interfaces/IWavsServiceHandler.sol)

It doesn't do very much does it! That's precisely the point - it's completely up to you for processing that data and handling it however you want. This is where you put all your business logic - no limits!