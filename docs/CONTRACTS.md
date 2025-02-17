When authoring custom services from outside the repo, you'll be using the CLI

See the [QUICKSTART](./QUICKSTART.md) for instructions on installing via Docker or natively. This document assumes you're up and running and can successfully start a chain and submit the example contracts from start to finish (specifically, you should have `anvil` and `wavs` up and running while you run the commands here).

A service is ultimately composed of a `Trigger`, `Component`, and `Submit`.

The `Trigger` and `Submit` are often Smart Contracts, and you're not limited whatsoever by following the WAVS examples.

# Triggers

First, build and deploy your trigger contract. This is actually outside the scope of anything WAVS cares about - you can use any contract you want for your trigger, it doesn't need to follow any known interface or satisfy any custom message type. You may also use any tooling you wish to deploy it. The important thing is to pay attention to the event you want to use for the trigger. On Ethereum, this will be the event signature (a.k.a. "topic 0") and on Cosmos it will be your event type (i.e. the `event.ty` field). We'll use this later when we tie everything together to deploy our service.

(TODO - explain how to derive this on Ethereum...}

# Submit

For the Submit target, we currently only support EigenLayer on Ethereum.

In the quickstart, this was deployed for you automatically. Now we're going to do this manually.

1. Deploy the Eigenlayer Core contracts, and register as an operator

```bash
wavs-cli deploy-eigen-core
```

You'll see a bunch of addresses, but you don't need to do anything with them, they're stashed in the CLI's deployment cache.

2. Deploy your custom Service Handler contract.

This is any contract you want, deployed however you want. The only thing that matters is that it satisfies the [IWavsServiceHandler interface in the sdk](../contracts/solidity/interfaces/IWavsServiceHandler.sol)

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

Now that you've deployed this contract using the tool of your choice (for example, `forge create`), make sure you have the address handy for the next step

3. Deploy the Eigenlayer Service Manager contract

In the previous step, you created a service _handler_. That is, it gets data that already came through and was vetted by the Eigenlayer.

In this step, we're creating the service _manager_. This will deal with all the integration between Eigenlayer and WAVS, and call your _handler_ after the data and signature has been verified.

The command to do this is:

```bash
wavs-cli deploy-eigen-service-manager --service-handler {{SERVICE_HANDLER_ADDR}}
```

**_As mentioned above, after this step is done, you might want to tell your handler about the _manager_ address - though this isn't a strict requirement_**

# Conclusion

Now you have your custom contracts, and, assuming you also have your component handy, you can deploy the service:

_see wavs-cli deploy-service --help for all the possibilities, this is a brief example_

```bash
wavs-cli deploy-service \
    --component path/to/my/component.wasm
    --trigger-event-name my-ethereum-event-signature
    --trigger-address my-ethereum-trigger-address
    --submit-address my-ethereum-submit-address
```
