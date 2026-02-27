# WAVS Components

## WIT Definitions, Go Bindings, and wasi-utils

### WIT Definitions

The `wit-definitions/` directory contains the [WebAssembly Interface Type](https://component-model.bytecodealliance.org/design/wit.html) definitions that describe the component interfaces:

- **`types/`** — shared types used across operator and aggregator interfaces (core types, chain configs, events, services)
- **`operator/`** — the operator component world (trigger processing)
- **`aggregator/`** — the aggregator component world (quorum handling and on-chain submission)
- **`wasi-tls/`** — custom WASI TLS extension interface

WIT dependencies are managed with [`wkg`](https://github.com/bytecodealliance/wasm-pkg-tools). Use `just wit-deps-fetch` to fetch dependencies and `just wit-build` to build WIT packages.

### Go Bindings

The `wasi/go/` directory contains auto-generated Go bindings from the WIT definitions. These provide Go-language access to the WAVS WASI interfaces (types, operator trigger world, and standard WASI APIs for HTTP, sockets, clocks, filesystem, etc.).

Module path: `github.com/Lay3rLabs/wavs/wasi/go`

### wasi-utils Crate

The `packages/wasi-utils/` directory contains the `wavs-wasi-utils` Rust crate, which provides utility functions for building WASI components:

- HTTP client utilities for making outbound requests from components
- EVM provider and event helpers for interacting with Ethereum chains
- Numeric conversion utilities

Published to [crates.io](https://crates.io/crates/wavs-wasi-utils) as `wavs-wasi-utils`.

### Using WIT Definitions in Your Own Project

Third-party component projects need a local copy of the WIT definitions to compile against. The general approach is:

1. **Clone the `wit-definitions/` directory** from this repo into your project
2. **Fetch WIT dependencies** with `wkg wit fetch`

Here is a minimal shell script example:

```bash
#!/usr/bin/env bash
set -euo pipefail

BRANCH="${1:-main}"
WIT_DIR="./wit-definitions"

# Clean and clone
rm -rf "$WIT_DIR" .temp-clone
mkdir -p .temp-clone
git -C .temp-clone clone --depth=1 --branch "$BRANCH" --single-branch https://github.com/Lay3rLabs/WAVS.git
cp -R .temp-clone/WAVS/wit-definitions "$WIT_DIR"
rm -rf .temp-clone

# Fetch dependencies
cd "$WIT_DIR/operator" && wkg wit fetch && cd -
cd "$WIT_DIR/aggregator" && wkg wit fetch && cd -
```

Your Rust component crate then references the local WIT directory via `wit-bindgen` as usual.

---

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

Each call can return **multiple** `WasmResponse` values. Each response becomes a separate signed submission. The `event_id_salt` is used to key aggregations — it isn't limited to distinguishing responses within a single trigger. It can be any arbitrary value chosen by the component, such as a Telegram message ID or any other domain-specific identifier, regardless of the trigger type.

### Registration

Export the component using the `export_layer_trigger_world!` macro:

```rust
use example_helpers::export_layer_trigger_world;
export_layer_trigger_world!(Component);
```

### Internal Test Components

The `examples/components/` directory contains components used for **internal testing only** — "examples" is a legacy name. Do not use them as a reference for building your own components: they rely on a shared `TriggerId` abstraction and common test infrastructure specific to this repo, which is not part of the core WAVS API and has historically caused confusion.

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

### Multiple Actions

Each entry point returns `Vec<AggregatorAction>`, so a single invocation can produce more than one action. For example, an aggregator can submit to multiple chains simultaneously, or return both a `Submit` and a `Timer` in the same response to post on-chain immediately and also schedule a follow-up callback.

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

### Internal Test Components

As with operator components, the aggregator components under `examples/components/` (`simple-aggregator`, `timer-aggregator`) are for internal testing only. See the note above about not using them as a reference.

---

## The Submit Enum

Each workflow in a service definition has a `submit` field that controls what happens after the operator component runs:

```rust
enum Submit {
    /// Execute the operator component but make no on-chain submission.
    /// The typical use-case is stashing local state in WASI key-value storage
    /// or the filesystem. Also valid when the component posts to an external
    /// API and no on-chain confirmation is needed.
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
