# Component Development Flow

Build, test, and deploy a new WAVS WASM component from scratch.

---

## Checklist

- [ ] **Step 1** — `wavs:wavs_get_wit_interface` — Read WIT definitions to understand available APIs before writing code.
- [ ] **Step 2** — `wavs:wavs_scaffold_component` — Generate project skeleton (`Cargo.toml` + `src/lib.rs`).
- [ ] **Step 3** — Implement logic in `src/lib.rs` using the patterns below.
- [ ] **Step 4** — `wavs:wavs_build_component` — Compile; read stderr and fix errors; repeat until exit code 0.
- [ ] **Step 5** — `wavs:wavs_upload_component` — Upload `.wasm`; save the returned digest (raw 64-char hex, no `sha256:` prefix).
- [ ] **Step 6** — `wavs:wavs_deploy_dev_service` (no on-chain contract) **or** follow [`deployment.md`](deployment.md) for a real deployment.
- [ ] **Step 7** — `wavs:wavs_simulate_trigger` — Verify output.

---

## Scaffold Parameters

```
name:         lowercase-with-hyphens  (e.g. "price-feed")
trigger_type: evm_contract_event | cosmos_contract_event | block_interval | cron | manual
description:  optional one-line description
```

Place the generated component at `examples/components/{name}/` to use workspace deps automatically.

---

## Component Anatomy

Minimal working component using the prelude:

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

The prelude re-exports: `Guest`, `TriggerAction`, `WasmResponse`, `decode_trigger_event`, `encode_trigger_output`, `export_layer_trigger_world`, and the `host` module.

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

For on-chain reads related to a trigger, use the trigger's block height as the canonical snapshot. Pass that block number/height to EVM, Cosmos, or RPC calls when supported, and avoid mixing historical trigger data with unpinned `latest` reads.

```rust
host::config_var("my-key")               // → Option<String>; reads service config
host::log(host::LogLevel::Info, "msg");  // levels: Debug, Info, Warn, Error
host::get_service()                      // → ServiceInfo (manager address, config, etc.)
host::get_event_id(None)                 // deterministic event ID (default salt)
host::get_event_id(Some(salt))           // custom salt (Vec<u8>)
```

---

## Trigger Type Patterns

```rust
match action.data {
    TriggerData::EvmContractEvent(_) | TriggerData::CosmosContractEvent(_) => {
        let (trigger_id, data) = decode_trigger_event(action.data)?;
    }
    TriggerData::Raw(bytes) => {
        // Plain bytes — trigger_id is 0 for raw/manual triggers.
        let data = bytes;
    }
    TriggerData::Cron { trigger_time } => {
        // trigger_time: unix timestamp (u64)
    }
    TriggerData::BlockInterval { block_height } => {
        // block_height: u64
    }
    _ => return Err("unsupported trigger type".to_string()),
}
```

---

## WASI APIs

### Key-Value Store

```rust
use example_helpers::bindings::world::wasi::keyvalue::{store, atomics};

let bucket = store::open("my-bucket").map_err(|e| e.to_string())?;
let value: Option<Vec<u8>> = bucket.get("key").map_err(|e| e.to_string())?;
bucket.set("key", &bytes).map_err(|e| e.to_string())?;
let new_val = atomics::increment(&bucket, "counter", 1).map_err(|e| e.to_string())?;
```

### Outbound HTTP

```rust
use wstd::runtime::block_on;

let bytes = block_on(async {
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
# Add for KV store or outbound HTTP:
# wstd = { workspace = true }
```

---

## Build Output Paths

After `wavs_build_component` (release mode):
```
target/wasm32-wasip1/release/{package_name_with_underscores}.wasm
```

After `just wasi-build-native {component-name}` (from repo root):
```
examples/build/components/{component-name}.wasm
```

Use the **absolute path** when calling `wavs_upload_component`.

---

## Debugging

**Build errors** (read `stderr` from `wavs_build_component`):
- `cannot find type` / `unresolved import` — check `example-helpers` path; try `use example_helpers::prelude::*`
- `the trait bound is not satisfied` — `encode_trigger_output` needs `&[u8]` or `AsRef<[u8]>`
- `does not implement Guest` — ensure `export_layer_trigger_world!(Component)` is present

**Runtime errors** (from `wavs_simulate_trigger`):
- Error message comes directly from your `?` or `return Err(...)` calls
- Add `host::log(host::LogLevel::Debug, &format!("data: {:?}", data))` and re-simulate

**Missing config vars** — component returns `"config var X not found"`:
- Service definition must include the key in its `config` map
- Verify with `wavs_get_service`

**Wrong trigger type** — `"unsupported trigger data type"`:
- The `match` must cover the trigger type configured in the service definition
- Use `TriggerData::Raw` for manual/simulation testing
