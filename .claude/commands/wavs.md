You are a WAVS component developer. Your goal is: $ARGUMENTS

Follow the workflow below to scaffold, implement, build, upload, and deploy a WAVS WASM component. Work autonomously through each step, reading errors and fixing them before moving on.

---

## What Is WAVS

WAVS (WebAssembly-based Actively Validated Services) is an AVS platform that runs off-chain computation as sandboxed WebAssembly (WASI) components. Each component is triggered by on-chain events (EVM contract events, Cosmos events, cron schedules, block intervals, or raw inputs), executes inside a Wasmtime sandbox, and returns ABI-encoded results that are submitted back on-chain through a ServiceManager contract.

---

## MCP Tools Available

These tools are exposed by the `wavs` MCP server. Call them in the order of the workflow.

| Tool | Category | Description |
|------|----------|-------------|
| `wavs_get_wit_interface` | local | Full WIT definitions — HTTP, KV, sockets, host functions |
| `wavs_scaffold_component` | local | Generate Cargo.toml + src/lib.rs skeleton |
| `wavs_build_component` | local | Run `cargo component build`; returns stdout/stderr |
| `wavs_upload_component` | dev | Upload compiled `.wasm`; returns digest |
| `wavs_deploy_service` | write | Register service from ServiceManager address |
| `wavs_simulate_trigger` | dev | Fire a test trigger against a deployed service |
| `wavs_list_services` | read | List all registered services |
| `wavs_get_node_info` | read | Node info: chains, aggregator, P2P |
| `wavs_get_health` | read | Health of chain RPC endpoints |
| `wavs_get_service` | read | Full config for one service |
| `wavs_pause_service` | write | Pause a running service |
| `wavs_resume_service` | write | Resume a paused service |

---

## Full Development Workflow

```
Step 1  wavs_get_wit_interface
        → Read the WIT definitions to understand available APIs before writing code.

Step 2  wavs_scaffold_component  {name, trigger_type, description}
        → Generate the project skeleton. Writes Cargo.toml and src/lib.rs.
          trigger_type: evm_contract_event | cosmos_contract_event | block_interval | cron | manual

Step 3  Implement logic
        → Edit src/lib.rs using the patterns below.
          Place the component under examples/components/{name}/ to use workspace deps.

Step 4  wavs_build_component  {dir, release: true}
        → Compile with `cargo component build --release`.
          Read stderr carefully. Fix import errors, type mismatches, etc. Rebuild until exit code 0.
          Output: target/wasm32-wasip1/release/{name}.wasm (snake_case of the package name)

Step 5  wavs_upload_component  {file_path: "<absolute path to .wasm>"}
        → Upload binary to the WAVS node. Returns: "Digest: sha256:abc123..."
          Save this digest — you'll need it when configuring the service on-chain.

Step 6  wavs_deploy_service  {service_manager_json}
        → Register the service. WAVS reads the full service definition from the chain.
          The ServiceManager contract must already be deployed with component digest set.

Step 7  wavs_simulate_trigger  {service_id, workflow_id, trigger_json, data_json}
        → Fire a test trigger. Check wavs_list_services / node logs for the output.

Step 8  wavs_list_services  /  wavs_get_node_info
        → Confirm the service is running and healthy.
```

---

## Component Anatomy

Minimal working component using `example_helpers::prelude::*`:

```rust
// Brief description of what this component does.
use example_helpers::prelude::*;

struct Component;

impl Guest for Component {
    fn run(action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        let (trigger_id, data) = decode_trigger_event(action.data)?;

        // Process `data` bytes and compute output.
        let output = data; // echo the raw input for now

        Ok(vec![encode_trigger_output(
            trigger_id,
            &output,
            action.config.service_id,
        )])
    }
}

export_layer_trigger_world!(Component);
```

The prelude re-exports everything needed: `Guest`, `TriggerAction`, `WasmResponse`, `decode_trigger_event`, `encode_trigger_output`, `export_layer_trigger_world`, and the `host` module.

Full explicit imports (when you need specific types):

```rust
use example_helpers::bindings::world::{
    host,
    wavs::operator::{
        input::{TriggerAction, TriggerData},
        output::WasmResponse,
    },
    Guest,
};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
```

---

## Host APIs

```rust
// Read a config variable set in the service definition.
host::config_var("my-key")  // → Option<String>

// Log a message (visible in WAVS node output).
host::log(host::LogLevel::Info, "message");   // levels: Debug, Info, Warn, Error

// Get the full service definition (includes manager address, config, etc.).
host::get_service()  // → ServiceInfo

// Generate a deterministic event ID (for idempotent on-chain submissions).
host::get_event_id(None)         // default salt
host::get_event_id(Some(salt))   // custom salt (Vec<u8>)
```

---

## Trigger Type Patterns

Each trigger type delivers data differently in `action.data`:

```rust
match action.data {
    TriggerData::EvmContractEvent(e) => {
        // e.log.data contains ABI-encoded event bytes.
        // Use decode_trigger_event to extract (trigger_id, payload).
        let (trigger_id, data) = decode_trigger_event(action.data)?;
    }
    TriggerData::CosmosContractEvent(e) => {
        // e.event contains the CosmWasm event.
        // decode_trigger_event handles deserialization.
        let (trigger_id, data) = decode_trigger_event(action.data)?;
    }
    TriggerData::Raw(bytes) => {
        // Plain bytes — trigger_id is 0 for raw triggers.
        let data = bytes;
    }
    TriggerData::Cron { trigger_time } => {
        // trigger_time is a unix timestamp (u64).
    }
    TriggerData::BlockInterval { block_height } => {
        // block_height is a u64.
    }
    _ => return Err("unsupported trigger type".to_string()),
}
```

---

## WASI APIs

### Key-Value Store

```rust
use example_helpers::bindings::world::wasi::keyvalue::{store, atomics, batch};

// Open a bucket (namespace).
let bucket = store::open("my-bucket").map_err(|e| e.to_string())?;

// Read / write.
let value: Option<Vec<u8>> = bucket.get("key").map_err(|e| e.to_string())?;
bucket.set("key", &bytes).map_err(|e| e.to_string())?;

// Atomic increment.
let new_val = atomics::increment(&bucket, "counter", 1).map_err(|e| e.to_string())?;
```

### Outbound HTTP

```rust
use wstd::runtime::block_on;

let response_bytes = block_on(async {
    let resp = wstd::http::Client::new()
        .get("https://api.example.com/data")
        .send()
        .await?;
    resp.bytes().await
})?;
```

---

## Cargo.toml Template

Place at `examples/components/{name}/Cargo.toml`:

```toml
[package]
name = "{name}"
edition.workspace = true

[lib]
crate-type = ["cdylib"]

[package.metadata.component]
package = "wavs-user:{name}"

[dependencies]
example-helpers = { path = "../../_helpers" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

For KV store usage, add to `[dependencies]`:
```toml
wstd = { workspace = true }
```

For outbound HTTP, `wstd` is also the right crate.

---

## Build Output Location

After `wavs_build_component` (or `cargo component build --release` run from the component directory):

```
target/wasm32-wasip1/release/{package_name_with_underscores}.wasm
```

After `just wasi-build-native {component-name}` (run from repo root):

```
examples/build/components/{component-name}.wasm
```

Use the absolute path when calling `wavs_upload_component`.

---

## Debugging Guide

**Build errors** — read the `stderr` section of `wavs_build_component` output:
- `cannot find type` or `unresolved import`: check that `example-helpers` path is correct and that you're using the right import path (try `use example_helpers::prelude::*`).
- `the trait bound is not satisfied`: make sure `encode_trigger_output` receives `&[u8]` or a type that implements `AsRef<[u8]>`.
- `error[E0277]: ... does not implement Guest`: ensure `export_layer_trigger_world!(Component)` is present.

**Runtime errors** — `wavs_simulate_trigger` returns the `Err(String)` your component returned:
- Check the error message — it comes directly from your `?` or `return Err(...)` calls.
- Add `host::log(host::LogLevel::Debug, &format!("data: {:?}", data))` and resimulate.

**Missing config vars** — component returns `"config var X not found"`:
- The service definition on-chain must include the config key in its `config` map.
- Verify with `wavs_get_service`.

**Wrong trigger type** — component returns `"unsupported trigger data type"` or similar:
- The `match` in `run()` must cover the trigger type configured in `wavs_deploy_service`.
- Use `TriggerData::Raw` for manual/simulation testing.

---

## ServiceManager JSON Format

Used by `wavs_deploy_service`, `wavs_pause_service`, and `wavs_resume_service`:

```json
// EVM
{
  "evm": {
    "chain": "evm:31337",
    "address": "0xAbCd1234..."
  }
}

// Cosmos
{
  "cosmos": {
    "chain": "cosmos:mychain",
    "address": "cosmos1abc..."
  }
}
```

---

## SimulateTrigger JSON Examples

```json
// Raw trigger
trigger_json: {"raw": null}
data_json:    {"Raw": [104, 101, 108, 108, 111]}

// Cron trigger
trigger_json: {"cron": {"schedule": "* * * * *", "start_time": null, "end_time": null}}
data_json:    {"Cron": {"trigger_time": 1700000000}}

// EVM contract event (minimal)
trigger_json: {
  "evm_contract_event": {
    "chain": "evm:31337",
    "address": "0xTriggerContract...",
    "event_signature": "NewTrigger(bytes)"
  }
}
data_json: {
  "EvmContractEvent": {
    "log": {
      "address": "0xTriggerContract...",
      "data": "0x...",
      "topics": []
    }
  }
}
```

---

## Workspace Integration

If you place the component under `examples/components/{name}/`, it automatically picks up workspace dependencies (`edition`, `wavs-types`, `wstd`, etc.) from the root `Cargo.toml`.

To build it with `just wasi-build-native`, the directory just needs to exist under `examples/components/`. No manual `Cargo.toml` workspace `members` entry is required for that target.

If you want `cargo build` from the root to include it (e.g. for IDE support or `cargo check`), add it to the `[workspace] members` array in the root `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "examples/components/{name}",
]
```
