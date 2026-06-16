
# WAVS Architecture

WAVS is a highly configurable AVS operator node. It allows you to easily run logic for multiple AVSs, with each service securely sandboxed from others and from the node's operating system.

## Sandboxing with WASI

WAVS uses [WASI](https://wasi.dev/interfaces) components to achieve strong isolation. Each AVS runs as a WASI component, with restricted access to system resources for security and reliability.

## Who Runs a WAVS Node?

WAVS nodes are operated by "Operators." These participants are similar to validators in proof-of-stake blockchains:

- Operators receive voting power through delegation.
- Their nodes perform off-chain actions, which are then submitted and verified on-chain.

To serve an AVS, an operator must explicitly opt-in. This is done via an HTTP endpoint on the node, where the operator provides the address and chain of the ServiceManager contract.

## Separation of Concerns

The AVS team is responsible for:
- Coding all WASI components
- Deploying AVS contracts on-chain

Operators and AVS teams are completely independent. AVS teams do not have access to operator systems, ensuring a clean separation of concerns.

## Internal Subsystems and Flow

At a high level, we have four subsystems:

1. **TriggerManager**: Monitors blockchain events, cron schedules, and block intervals; fires trigger events to the Dispatcher.
2. **Engine**: Executes the WASM components inside a Wasmtime WASI sandbox; returns the operator response.
3. **SubmissionManager**: Signs the operator response with the service's derived key and hands the signed submission to the Aggregator.
4. **Aggregator**: Collects signatures from peer operators via P2P, runs the aggregator component once quorum is reached, and submits the result on-chain.

These each run in their own thread and are orchestrated by the Dispatcher, typically over crossbeam unbounded channels.

Administration of a node is done through an HTTP server, which holds an instance of the dispatcher and can call it directly.

As a rule of thumb, we do not block — async tasks are spawned onto the Tokio runtime — and we do not crash — errors are logged via the tracing mechanism and inspected via tools like Jaeger.

### Full Execution Flow

```
TriggerManager → Dispatcher → Engine → SubmissionManager → Aggregator → Chain
```

1. **TriggerManager** fires a `TriggerAction` to the Dispatcher when a watched event or schedule fires.
2. **Dispatcher** routes the action to the **Engine**, which executes the matching operator WASM component.
3. The **Engine** returns a `WasmResponse`; the Dispatcher forwards it to the **SubmissionManager**.
4. **SubmissionManager** signs the response and produces a `Submission`, which is handed back to the Dispatcher.
5. The Dispatcher sends the `Submission` to the **Aggregator**, which broadcasts it to peer operators via P2P.
6. Once the quorum threshold is met, the Aggregator invokes the workflow's aggregator WASM component (via the Engine), which returns `AggregatorAction` values.
7. The Aggregator posts the result on-chain (EVM or Cosmos service handler contract).

### Submit::None Short-Circuit

When a workflow's `submit` field is set to `None`, execution stops after step 3. The operator component runs and can produce any side effects (e.g. posting data to an external API), but nothing is signed or submitted on-chain. No service handler contract is required.

### Multi-Operator Quorum

Submissions from multiple operators accumulate in a `QuorumQueue` keyed by `(EventId, SubmitAction)`. When the required threshold of signatures is collected, the aggregator component is invoked with the full set, allowing it to validate or select among the collected signatures before posting.

Burned (submitted) queues are retained for a configurable TTL (default 48 hours) and then cleaned up.

## Supported Trigger Types

| Trigger | Description |
|---|---|
| `EvmContractEvent` | EVM contract log event (filtered by event signature hash) |
| `CosmosContractEvent` | Cosmos contract event (filtered by event type string) |
| `BlockInterval` | Every N blocks on a specified EVM or Cosmos chain |
| `Cron` | Cron expression schedule (with optional start/end timestamps) |
| `AtProto` / Jetstream | ATProto Jetstream events (Bluesky, filtered by collection NSID) |
| `Hypercore` | Appends to a Hypercore feed (identified by feed key) |
| `Manual` | HTTP endpoint trigger (dev/testing only) |

## P2P Networking

The Aggregator subsystem includes optional P2P networking for multi-operator deployments. Operators broadcast signed submissions via a commonware broadcast channel with application-level service filtering. Peer connectivity uses either lookup mode (local/dev, known addresses) or discovery mode (production, bootstrapper-based). Identities are Ed25519 keypairs derived from the signing mnemonic. See [P2P.md](P2P.md) for configuration details.
