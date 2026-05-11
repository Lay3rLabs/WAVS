# Component Development Flow

Build, test, and deploy a new WAVS WASM component from scratch.

---

## Checklist

- [ ] **Step 1** — `wavs:wavs_scaffold_component` with `dir` parameter — Creates the complete project on disk with all WIT files, bindings, and trigger template. Nothing to write manually.
- [ ] **Step 2** — Customize `src/lib.rs` with your component logic.
- [ ] **Step 3** — `wavs:wavs_build_component` — Compile; read stderr and fix errors; repeat until exit code 0.
- [ ] **Step 4** — `wavs:wavs_validate_component` — Verify the .wasm exports the correct `run` function before uploading.
- [ ] **Step 5** — `wavs:wavs_upload_component` — Upload `.wasm`; save the returned digest (raw 64-char hex, no `sha256:` prefix).
  - **OCI alternative:** If the component is published to an OCI registry (e.g. ghcr.io), you can skip upload and use an OCI source in the service definition instead: `"source": {"oci": {"uri": "oci://ghcr.io/org/component:v1.0"}}`. See [`reference/service-json.md`](../reference/service-json.md#oci-pull-from-registry-at-deploy-time) for details.
- [ ] **Step 6** — `wavs:wavs_deploy_dev_service` (no on-chain contract) **or** follow [`deployment.md`](deployment.md) for a real deployment.
- [ ] **Step 7** — If the `ui_navigate` tool is available (WAVS desktop app embedded agent only), **immediately** call it to open the service detail page (path from deploy output). Don't wait — navigate right after deploy so the user can see the service.
- [ ] **Step 8** — `wavs:wavs_simulate_trigger` — Verify output.

> **Tip:** Call `wavs:wavs_get_wit_interface` if you need to understand the full WIT API (HTTP, KV, host functions, etc.) before writing custom logic.
> **Tip:** Omit the `dir` parameter from `wavs_scaffold_component` to get file contents as text instead of writing to disk (useful when integrating into existing projects).

---

## How Scaffolding Works

**With `dir` parameter (recommended):** The tool creates `{dir}/{name}/` with all files ready to build. No manual file creation needed.

**Without `dir`:** Returns file contents as text. You must write every file yourself, including the `wit/` directory. Use this only when integrating into an existing project.

The scaffolded project is self-contained:
- Builds with `cargo build --target wasm32-wasip2 --release` (no `cargo-component` needed)
- All WIT interface definitions are bundled in `wit/`
- **Prerequisite:** `rustup target add wasm32-wasip2`

### In-Workspace Alternative (WAVS repo only)

If working inside the WAVS monorepo, you can instead create a component at `examples/components/{name}/` using `example-helpers = { workspace = true }`. This is simpler (only 2 files, no WIT copy needed) but only works within the workspace. Build with `cargo component build --release -p {name}`.

---

## Scaffold Parameters

```
name:         lowercase-with-hyphens  (e.g. "price-feed")
trigger_type: evm_contract_event | cosmos_contract_event | block_interval | cron | manual
description:  optional one-line description
```

---

## Component Anatomy (In-Workspace)

Minimal working component using the prelude:

```rust
// Brief description of what this component does.
use example_helpers::prelude::*;

struct Component;

impl Guest for Component {
    fn run(action: TriggerAction) -> Result<Vec<WasmResponse>, String> {
        match action.data {
            TriggerData::Raw(data) => {
                // Process raw input bytes
                Ok(vec![WasmResponse {
                    payload: data,
                    ordering: None,
                    event_id_salt: None,
                }])
            }
            _ => Err("Unsupported trigger data type".to_string()),
        }
    }
}

export_layer_trigger_world!(Component);
```

The prelude re-exports: `Guest`, `TriggerAction`, `TriggerData`, `Trigger`, `WasmResponse`, `decode_trigger_event`, `encode_trigger_output`, `export_layer_trigger_world`, and the `host` module.

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

## Component Anatomy (Standalone)

```rust
#[allow(warnings)]
mod bindings;

use crate::bindings::{
    export,
    wavs::types::events::TriggerData,
    Guest, TriggerAction, WasmResponse,
};

struct Component;
export!(Component with_types_in bindings);

impl Guest for Component {
    fn run(action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {
        match action.data {
            TriggerData::Raw(data) => {
                let payload = data; // echo input back
                Ok(vec![WasmResponse {
                    payload,
                    ordering: None,
                    event_id_salt: None,
                }])
            }
            _ => Err("Unsupported trigger data type".to_string()),
        }
    }
}
```

With `src/bindings.rs`:
```rust
#[allow(warnings)]
mod _inner {
    wit_bindgen::generate!({
        world: "wavs-world",
        path: "wit",
        pub_export_macro: true,
        generate_all,
        features: ["tls"],
    });
}
pub use _inner::*;
```

---

## Host APIs

```rust
// In-workspace: available via `host::` directly
// Standalone: use `crate::bindings::host`
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
    TriggerData::EvmContractEvent(event_data) => {
        // event_data.chain: chain key string
        // event_data.log.data.data: raw ABI-encoded log bytes
        // event_data.log.data.topics: Vec of topic byte arrays
    }
    TriggerData::CosmosContractEvent(event_data) => {
        // event_data.chain: chain key string
        // event_data.event.ty: event type string
        // event_data.event.attributes: Vec<(String, String)>
        // event_data.block_height: u64
    }
    TriggerData::Raw(bytes) => {
        // Plain bytes — for manual/raw triggers
    }
    TriggerData::Cron(data) => {
        // data.trigger_time.nanos: unix timestamp in nanoseconds
    }
    TriggerData::BlockInterval(data) => {
        // data.block_height: u64
        // data.chain: chain key string
    }
    _ => return Err("unsupported trigger type".to_string()),
}
```

---

## WASI APIs

### Key-Value Store

```rust
use example_helpers::bindings::world::wasi::keyvalue::{store, atomics};
// Standalone: use crate::bindings::wasi::keyvalue::{store, atomics};

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

## Cargo.toml Template (In-Workspace)

Place at `examples/components/{name}/Cargo.toml`:

```toml
[package]
name = "{name}"
edition.workspace = true
version.workspace = true
authors.workspace = true
rust-version.workspace = true
repository.workspace = true

[lib]
crate-type = ["cdylib"]

[package.metadata.component]
package = "component:{name}"

[dependencies]
example-helpers = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
# Uncomment for async HTTP, timers, etc:
# wstd = { workspace = true }
```

---

## Build Output Paths

After `wavs_build_component` or `cargo component build --release` (workspace):
```
target/wasm32-wasip1/release/{package_name_with_underscores}.wasm
```

After `cargo build --target wasm32-wasip2 --release` (standalone):
```
target/wasm32-wasip2/release/{package_name_with_underscores}.wasm
```

After `just wasi-build-native {component-name}` (from repo root):
```
examples/build/components/{component-name}.wasm
```

Use the **absolute path** when calling `wavs_upload_component`.

---

## Debugging

**Build errors** (read `stderr` from `wavs_build_component`):
- `cannot find type` / `unresolved import` — check import paths match your scaffold mode (workspace vs standalone)
- `failed to create a target world` / `package not found` — WIT files are missing. For standalone: ensure all `wit/deps/*/package.wit` files are written. For workspace: ensure `example-helpers` path is correct.
- `the trait bound is not satisfied` — `encode_trigger_output` needs `&[u8]` or `AsRef<[u8]>`
- `does not implement Guest` — ensure export macro is present (`export_layer_trigger_world!` for workspace, `export!` for standalone)
- `no export 'run' found` — the export macro is missing or the `Guest` impl is not correct

**Runtime errors** (from `wavs_simulate_trigger`):
- Error message comes directly from your `?` or `return Err(...)` calls
- Add `host::log(host::LogLevel::Debug, &format!("data: {:?}", data))` and re-simulate
- Use `wavs_query_component_logs(service_id="<id>", level="debug")` to read component `host::log()` output after simulation
- Use `wavs_query_logs(target="wavs::subsystems::engine", level="warn")` for broader engine-level diagnostics

**Missing config vars** — component returns `"config var X not found"`:
- Service definition must include the key in its `config` map
- Verify with `wavs_get_service`

**Wrong trigger type** — `"unsupported trigger data type"`:
- The `match` must cover the trigger type configured in the service definition
- Use `TriggerData::Raw` for manual/simulation testing
