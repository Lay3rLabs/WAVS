When authoring custom services from outside the repo, you'll be using the CLI

A service is ultimately composed of Workflows, which each contain a `Trigger`, `Component`, and `Submit`.

The `Trigger` and `Submit` are often Smart Contracts, and you're not limited whatsoever by following the WAVS examples.

# Triggers

This is actually outside the scope of anything WAVS cares about - you can use any contract you want for your trigger, it doesn't need to follow any known interface or satisfy any custom message type. You may also use any tooling you wish to deploy it. The important thing is to pay attention to the event you want to use for the trigger. On EVM, this will be the event signature (a.k.a. "topic 0") and on Cosmos it will be your event type (i.e. the `event.ty` field).

When you create a service with a contract event trigger, you simply tell WAVS the contract address and event.

# Submit

For the Submit target, when targetting Eigenlayer, your contract needs to satisfy the [IWavsServiceHandler interface](../contracts/solidity/interfaces/IWavsServiceHandler.sol)

Let's take a look at it:

```Solidity
interface IServiceHandler {
    /**
     * @notice Called by WavsServiceManager after successful payload signature validation.
     * @param data The arbitrary data that was signed.
     * @param signature The signature of the data.
     */
    function handleAddPayload(bytes calldata data, bytes calldata signature) external;
}
```

It doesn't do very much does it! That's precisely the point - it's completely up to you for processing that data and handling it however you want. This is where you put all your business logic - no limits!

_tip: be careful with security here! A good pattern is to have a guard of some sort that makes sure the only one allowed to call this function is the ServiceManager. You can see this in the [example contract](../examples/contracts/solidity/SimpleSubmit.sol)_
