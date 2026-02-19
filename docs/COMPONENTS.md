# WAVS Components

WAVS has two distinct component types that serve different roles in the execution pipeline. Both are compiled to WebAssembly using the [Component Model](https://component-model.bytecodealliance.org/) spec and run inside a Wasmtime WASI sandbox.

---

## Operator Components

Operator components are the primary building block of a WAVS service. They execute in response to a trigger event and produce a signed payload that gets submitted to the aggregation pipeline.

### Interface

Operator components implement the `Guest` trait generated from the operator WIT world:

```rust
impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        // your logic here
    }
}
```

The `TriggerAction` contains both the trigger configuration (service ID, workflow ID, trigger type) and the raw trigger data (decoded event, block, cron tick, etc.).

Each call can return **multiple** `WasmResponse` values. Each response becomes a separate signed submission. If multiple responses are returned, each must supply a unique `event_id_salt` to distinguish them.

### Registration

Export the component using the `export_layer_trigger_world!` macro:

```rust
use example_helpers::export_layer_trigger_world;
export_layer_trigger_world!(Component);
```

### Examples

- `examples/components/echo-data` — echoes trigger data, demonstrates multiple trigger types
- `examples/components/echo-block-interval` — responds to block interval triggers
- `examples/components/echo-cron-interval` — responds to cron schedule triggers
- `examples/components/kv-store` — demonstrates WASI key-value store access
- `examples/components/square` — minimal example with simple computation

---

## Aggregator Components

Aggregator components run **after** quorum is reached across multiple operator submissions. Rather than processing the raw trigger event, they receive the collected operator result and decide how (and when) to submit it on-chain.

### When They Run

After the threshold of operators have signed and submitted a `WasmResponse` for a given event, the aggregator component is invoked via the `process_input` entry point. It can then either submit immediately or schedule a timer for deferred submission.

### Interface

Aggregator components implement the `Guest` trait generated from the aggregator WIT world, which has three entry points:

```rust
impl Guest for Component {
    /// Called when quorum is first reached for an event.
    /// Return Submit to post on-chain immediately, or Timer to defer.
    fn process_input(input: AggregatorInput) -> Result<Vec<AggregatorAction>, String>;

    /// Called when a previously set timer fires.
    /// Typically returns a Submit action at this point.
    fn handle_timer_callback(input: AggregatorInput) -> Result<Vec<AggregatorAction>, String>;

    /// Called after an on-chain submission completes (success or failure).
    /// Return value is ignored; use for logging or cleanup only.
    fn handle_submit_callback(
        input: AggregatorInput,
        tx_result: Result<AnyTxHash, String>,
    ) -> Result<(), String>;
}
```

All three entry points receive the same `AggregatorInput`, which bundles the original `TriggerAction` with the `WasmResponse` from the operator phase:

```rust
struct AggregatorInput {
    trigger_action: TriggerAction,
    operator_response: WasmResponse,
}
```

When quorum is met, the aggregator component receives the full signed submission set, allowing it to validate or select among the collected operator signatures.

### AggregatorAction Variants

```rust
enum AggregatorAction {
    /// Post the result on-chain now.
    Submit(SubmitAction),
    /// Wait for the given duration, then call handle_timer_callback.
    Timer(TimerAction),
}

enum SubmitAction {
    Evm(EvmSubmitAction),      // chain, service handler address, optional gas price
    Cosmos(CosmosSubmitAction), // chain, contract address (bech32), optional gas price
}

struct TimerAction {
    delay: Duration,
}
```

### Registration

Export the component using the `export_aggregator_world!` macro:

```rust
export_aggregator_world!(Component);
```

### Examples

- `examples/components/simple-aggregator` — submits immediately in `process_input`; supports both EVM and Cosmos targets; optionally fetches gas price from an oracle
- `examples/components/timer-aggregator` — defers submission via a `Timer` action in `process_input`, then submits in `handle_timer_callback`; validates trigger data before committing

---

## The Submit Enum

Each workflow in a service definition has a `submit` field that controls what happens after the operator component runs:

```rust
enum Submit {
    /// Execute the operator component but make no on-chain submission.
    /// Useful when the component performs side effects (e.g. posting to an API)
    /// or when on-chain confirmation is not required.
    None,

    /// After quorum is reached, run the specified aggregator component
    /// to determine the on-chain submission.
    Aggregator {
        component: Component,
        signature_kind: SignatureKind,
    },
}
```

`Submit::None` is valid whenever the operator component's output is self-contained and no service handler contract is needed.

`Submit::Aggregator` requires a deployed service handler contract on the target chain — see [CONTRACTS.md](CONTRACTS.md).

---

## Component Lifecycle Summary

```
Trigger fires
     │
     ▼
Operator component (export_layer_trigger_world!)
  run(TriggerAction) → Vec<WasmResponse>
     │
     │  if submit = None: stop here, nothing posted on-chain
     │
     ▼  if submit = Aggregator:
Signed by operator, broadcast via P2P
     │
     ▼  quorum threshold reached
Aggregator component (export_aggregator_world!)
  process_input(AggregatorInput) → Vec<AggregatorAction>
     │
     ├─ AggregatorAction::Submit → post on-chain → handle_submit_callback
     └─ AggregatorAction::Timer  → wait → handle_timer_callback → post on-chain
```
