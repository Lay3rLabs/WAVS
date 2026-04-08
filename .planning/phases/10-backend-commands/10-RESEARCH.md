# Phase 10: Backend Commands - Research

**Researched:** 2026-04-08
**Domain:** Tauri commands, wit-schema, WASM component introspection, Rust
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Command Design**
- Two separate Tauri commands: one for JSON Schema (interface/exports), one for metadata (permissions, limits, config)
- Both commands accept a component digest string as input parameter
- Load component bytes from existing component store using digest (reuse existing storage layer)
- Use `generate_schema_cached` from wit-schema for LRU caching of parsed schemas

**Response Shape**
- Schema command returns wit-schema's JSON Schema output directly (D-04 spec: `world`, `exports`, `$defs`)
- Metadata command returns a flat struct with typed fields mirroring Component: `permissions`, `fuel_limit`, `time_limit_seconds`, `config`, `env_keys`, `source`
- Source info includes source type + variant-specific fields (URI for Download/OCI, registry for Registry, raw digest for Digest)

**Error Handling & Edge Cases**
- Component not found returns typed error with "component not found" message via existing AppResult pattern
- wit-schema parse failure returns error with parse failure details for debugging
- Component with no exports returns valid schema with empty exports array (technically correct)

### Claude's Discretion

No items deferred to Claude's discretion — all areas accepted as recommended.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BACK-01 | User can retrieve JSON Schema (exported functions, input/output types, doc comments) for a component via Tauri command | `wit_schema::generate_schema_cached` is ready; needs Tauri command wrapper + Dispatcher method to retrieve bytes |
| BACK-02 | User can retrieve component metadata (permissions, resource limits, config keys, env vars) via Tauri command | `Component` struct in wavs-types has all required fields; needs Tauri command wrapper + service scan by digest |
</phase_requirements>

## Summary

Phase 10 adds two Tauri commands that expose component introspection data to the frontend. The `wit-schema` crate (`packages/wit-schema/`) already provides `generate_schema_cached` with LRU caching — the schema command is primarily a wiring task. The metadata command reads fields directly from the `Component` struct in `wavs-types`.

The critical discovery is that the `Dispatcher` (the primary backend entry point for commands) currently has no method to retrieve raw WASM bytes by digest. `WasmEngine.engine` (a `BaseEngine<S>`) is a private field, so `BaseEngine.storage.get_data()` cannot be reached from outside. A new method must be added to `WasmEngine` and a corresponding forwarding method to `Dispatcher`. The metadata command also needs a service-scan-by-digest approach since no `services.get_by_digest()` API exists.

Both commands share a `SchemaCacheState` (new Tauri managed state wrapping `wit_schema::SchemaCache`), registered in `lib.rs` alongside existing managed states.

**Primary recommendation:** Add `get_component_bytes_and_engine(digest)` to `WasmEngine`, forward via `Dispatcher`, register `SchemaCacheState`, then implement both commands following the established `#[tauri::command(rename_all = "snake_case")]` pattern.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `wit-schema` | workspace | WIT-to-JSON-Schema generation with LRU cache | Already in this workspace; decision locked |
| `wasmtime` | workspace | WASM component type introspection | Required by wit-schema; already a workspace dep |
| `wavs-types` | workspace | `Component`, `ComponentSource`, `Permissions`, `ComponentDigest` | Shared types across all packages |
| `wavs-gui-shared` | workspace | `AppResult<T>` / `AppError` error handling | Established pattern for all Tauri commands |
| `serde_json` | workspace | `Value` — JSON Schema output from wit-schema | Already a dep |
| `serde` | workspace | `Serialize` / `Deserialize` for response structs | Already a dep |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tauri::State` | workspace | Injecting managed state into commands | All stateful commands |
| `lru` | workspace | Used internally by `SchemaCache` | Indirect — no direct use in commands |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `generate_schema_cached` | `generate_schema` directly | No caching — every call recompiles WASM type info; worse perf |
| `SchemaCache` as Tauri state | Per-call cache | LRU across calls; 32-entry default handles common workloads |

### Cargo dependency to add

`wit-schema` is **not** currently in `app/src-tauri/Cargo.toml`. It must be added: [VERIFIED: codebase grep]

```toml
# app/src-tauri/Cargo.toml
wit-schema = { workspace = true }
```

`wavs` (already a dep) does NOT re-export `wit-schema`. [VERIFIED: codebase grep of `packages/wavs/Cargo.toml`]

## Architecture Patterns

### Established Tauri Command Pattern

[VERIFIED: codebase inspection of `app/src-tauri/src/commands.rs`]

```rust
// commands.rs — declaration
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_component_schema(
    wavs_instance: State<'_, WavsInstanceState>,
    schema_cache: State<'_, SchemaCacheState>,
    digest: String,
) -> AppResult<serde_json::Value> { ... }

// lib.rs — registration (two places: import + invoke_handler)
use crate::commands::{..., cmd_get_component_schema, cmd_get_component_metadata};

tauri::generate_handler![
    // ... existing commands ...
    cmd_get_component_schema,
    cmd_get_component_metadata,
]
```

### New Managed State

`SchemaCache` from `wit-schema` must be wrapped in a Tauri managed state and registered in `lib.rs` setup. Following the pattern of `WavsInstanceState` and `McpServerState`:

```rust
// state.rs — new state type
pub struct SchemaCacheState {
    pub inner: wit_schema::SchemaCache,
}

impl Default for SchemaCacheState {
    fn default() -> Self {
        Self { inner: wit_schema::SchemaCache::default() }
    }
}

// lib.rs — register during setup
app.manage(SchemaCacheState::default());
```

### Required New Method: WasmEngine::get_component_bytes

The schema command needs raw WASM bytes + a `wasmtime::component::Component`. Currently, `WasmEngine.engine` (a `BaseEngine<S>`) is a **private** field. `BaseEngine.storage.get_data(digest)` retrieves bytes, but is not reachable from `Dispatcher` externally. [VERIFIED: codebase inspection]

A new method must be added to `WasmEngine`:

```rust
// packages/wavs/src/subsystems/engine/wasm_engine.rs
pub async fn get_component_bytes(&self, digest: &ComponentDigest)
    -> Result<Vec<u8>, EngineError>
{
    self.engine.storage
        .get_data(&digest.clone().into())
        .map_err(|e| EngineError::StorageError(format!("Component not found: {}", e)))
}
```

And forwarded from `Dispatcher`:

```rust
// packages/wavs/src/dispatcher.rs
pub async fn get_component_bytes(&self, digest: &ComponentDigest)
    -> Result<Vec<u8>, DispatcherError>
{
    Ok(self.engine_manager.engine.get_component_bytes(digest).await?)
}
```

The `wasmtime::Engine` handle is at `dispatcher.engine_manager.engine.engine.wasm_engine` — but `engine` is private on `WasmEngine`. The command will need to either:
- **Option A (preferred):** Add `get_component_bytes_and_engine` that returns `(Vec<u8>, WasmEngine_ref)` plus expose the `wasmtime::Engine` via a getter — minimal surface
- **Option B:** Add `get_component_schema(digest)` directly on `WasmEngine` (encapsulates all wit-schema logic in the backend package, keeps commands.rs thin)

Option B is cleaner: the wit-schema call lives next to the engine, not in the GUI command layer. However it requires adding `wit-schema` as a dependency of `packages/wavs`. Option A keeps `wit-schema` only in the GUI layer.

**Recommendation: Option A** — `WasmEngine` exposes `get_component_bytes(digest) -> Vec<u8>` and a `wasmtime_engine()` getter returning `&wasmtime::Engine`. The Tauri command calls both, constructs the `wasmtime::component::Component`, then calls `generate_schema_cached`. This keeps `wit-schema` out of the core engine package.

### Metadata Command: Finding Component by Digest

No `services.get_by_digest(digest)` API exists. The metadata command must: [VERIFIED: codebase inspection]

1. Parse the digest string to `ComponentDigest` via `ComponentDigest::from_str()`
2. Call `dispatcher.services.list(Unbounded, Unbounded)`
3. Find the first service whose `workflow.component.source.digest() == Some(&target_digest)`
4. If found: map `Component` fields to `ComponentMetadataResult` response struct
5. If not found: verify the digest exists in component storage; if yes, return metadata with defaults (empty permissions/config); if no, return `AppError::Service("component not found: {digest}")`

### Response Struct for Metadata

```rust
#[derive(Serialize)]
pub struct ComponentMetadataResult {
    pub permissions: wavs_types::Permissions,
    pub fuel_limit: Option<u64>,
    pub time_limit_seconds: Option<u64>,
    pub config: std::collections::BTreeMap<String, String>,
    pub env_keys: std::collections::BTreeSet<String>,
    pub source: ComponentSourceResult,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComponentSourceResult {
    Download { uri: String, digest: String },
    Registry { digest: String, domain: Option<String>, package: String },
    Digest { digest: String },
    Oci { uri: String, digest: Option<String> },
}
```

### Anti-Patterns to Avoid

- **Calling `generate_schema` (uncached):** Always use `generate_schema_cached` — WASM type introspection is non-trivial CPU work.
- **Constructing a fresh `wasmtime::Engine` per call:** Reuse the existing engine from the running WAVS instance. Creating a new Engine is expensive.
- **Returning raw `DispatcherError` from commands:** Map errors to `AppError::Service(...)` for consistent frontend behavior.
- **Forgetting to register the new state in `lib.rs`:** `SchemaCache` as an unmanaged field will panic at runtime when injected as `State<'_>`.
- **Forgetting to add `wit-schema` to `app/src-tauri/Cargo.toml`:** This is the most likely compile-time oversight.
- **Not registering commands in `lib.rs` invoke_handler:** Commands declared in `commands.rs` but not in `tauri::generate_handler![]` will silently fail at runtime (Tauri returns "command not found" error).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WIT-to-JSON-Schema | Custom type visitor | `wit_schema::generate_schema_cached` | Handles recursive types, shared `$defs`, optional doc enrichment, LRU cache |
| LRU schema cache | `HashMap` or `Mutex<HashMap>` | `wit_schema::SchemaCache` | Already implemented, tested, thread-safe |
| Component digest parsing | Manual hex decode | `ComponentDigest::from_str(s)` | Uses `const_hex`, matches stored digest format exactly |
| Error type | New error enum | `AppError::Service(String)` | Consistent with all other commands; serializes properly to frontend |

**Key insight:** `wit-schema` was built specifically for this use case in v1.0. The only work is wiring, not implementation.

## Common Pitfalls

### Pitfall 1: Missing `wit-schema` in Cargo.toml

**What goes wrong:** Compilation error — `error[E0433]: failed to resolve: use of undeclared crate or module 'wit_schema'`
**Why it happens:** `wit-schema` is a workspace member but not in `app/src-tauri/Cargo.toml`. The `wavs` crate (already a dep) does not re-export it.
**How to avoid:** Add `wit-schema = { workspace = true }` to `[dependencies]` before writing any code.
**Warning signs:** Compiler error mentioning `wit_schema` not found.

### Pitfall 2: Reusing a Component Object Compiled for a Different Engine

**What goes wrong:** `wasmtime` panics if a `Component` is used with an `Engine` other than the one it was compiled against.
**Why it happens:** Each `wasmtime::Engine` has its own internal state; `Component::new(engine, bytes)` compiles for that specific engine.
**How to avoid:** Always pass the `wasmtime::Engine` from the same running WAVS instance (the one inside `WasmEngine`). Do not create a fresh `wasmtime::Engine` for schema generation.
**Warning signs:** Runtime panic in `generate_schema` mentioning "store belongs to a different engine."

### Pitfall 3: Component Not in Store When WAVS is Not Running

**What goes wrong:** Command is called while WAVS is stopped — `dispatcher()` returns `AppError::WavsNotRunning`.
**Why it happens:** `WavsInstanceState.dispatcher()` returns `Err(AppError::WavsNotRunning)` if no instance is active.
**How to avoid:** The commands correctly propagate the `?` — no special handling needed. The frontend must handle this error state.
**Warning signs:** Frontend receives `WavsNotRunning` error on schema/metadata commands.

### Pitfall 4: Metadata for Components Not Attached to a Service

**What goes wrong:** A component was uploaded (digest exists in storage) but never added to a service — `services.list()` scan finds nothing.
**Why it happens:** The component store is independent of the service registry. A digest can exist without any service referencing it.
**How to avoid:** The scan-by-digest approach must fall back: if digest is in `list_component_digests()` but no service references it, return metadata with all-default values (empty permissions, no limits, no config). This is correct per spec — a just-uploaded component has no service-assigned permissions yet.
**Warning signs:** Metadata command returns errors for components shown in the component list.

### Pitfall 5: Forgetting to Register `SchemaCacheState` in lib.rs

**What goes wrong:** Tauri panics at startup: "state not managed for field type"
**Why it happens:** Tauri's `State<'_, T>` injection requires `app.manage(T)` to be called during setup.
**How to avoid:** Add `app.manage(SchemaCacheState::default())` to the `setup` closure in `lib.rs`.
**Warning signs:** Application crashes immediately after the window is created.

## Code Examples

### wit-schema API Verified Signature

[VERIFIED: `/workspace/packages/wit-schema/src/lib.rs`]

```rust
// Full signature of generate_schema_cached
pub fn generate_schema_cached(
    engine: &wasmtime::Engine,
    component: &wasmtime::component::Component,
    wasm_bytes: &[u8],
    options: &SchemaOptions,
    cache: &SchemaCache,
) -> anyhow::Result<serde_json::Value>
```

Output shape (D-04 spec):
```json
{
  "world": "unknown",
  "exports": {
    "func-name": {
      "inputSchema": { ... },
      "outputSchema": { ... }
    }
  },
  "$defs": { ... }
}
```

### SchemaCache Default

[VERIFIED: `/workspace/packages/wit-schema/src/cache.rs`]

```rust
// Default capacity: 32 entries, keyed by ComponentDigest
let cache = SchemaCache::default();
```

### ComponentDigest Parsing

[VERIFIED: `/workspace/packages/types/src/id/hash.rs`]

```rust
use std::str::FromStr;
use wavs_types::ComponentDigest;

let digest = ComponentDigest::from_str(&digest_string)
    .map_err(|e| AppError::Service(format!("Invalid digest: {}", e)))?;
```

### AppResult Error Mapping Pattern

[VERIFIED: `/workspace/packages/gui/shared/src/error.rs`]

```rust
.map_err(|e| AppError::Service(format!("Component not found: {}", e)))?
```

### Existing Component Digest Lookup Pattern

[VERIFIED: `/workspace/app/src-tauri/src/commands.rs` around line 669]

```rust
let digest = wavs_instance
    .dispatcher()?
    .store_component_bytes(bytes)
    .map_err(|e| AppError::Service(format!("Failed to store component: {}", e)))?;
```

### Accessing Service List for Metadata Scan

[VERIFIED: `/workspace/app/src-tauri/src/commands.rs` around line 283]

```rust
wavs_instance
    .dispatcher()?
    .services
    .list(std::ops::Bound::Unbounded, std::ops::Bound::Unbounded)
    .map_err(|e| AppError::Service(e.to_string()))
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No schema API | `wit_schema::generate_schema_cached` with LRU | v1.0 milestone | Schema generation is ready; this phase only wires it |

**No deprecated patterns** in this domain — `wit-schema` is new and has no legacy API.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Adding `get_component_bytes` to `WasmEngine` is the correct approach (rather than adding `wit-schema` dep to `packages/wavs`) | Architecture Patterns | Low — both options work; Option A is cleaner but either is acceptable |
| A2 | Components with no associated service should return default metadata rather than an error | Common Pitfalls (Pitfall 4) | Medium — if the spec intends "component not found in any service = error", the fallback approach would be wrong |

## Open Questions

1. **WasmEngine exposure approach: Option A vs B**
   - What we know: `WasmEngine.engine` is private; `BaseEngine.storage` is public
   - What's unclear: Whether adding wit-schema as a dep of `packages/wavs` is acceptable (would enable encapsulating schema logic in engine layer)
   - Recommendation: Default to Option A (expose bytes + engine getter from WasmEngine, call wit-schema in Tauri commands layer). If the team prefers no wit-schema in the GUI layer, Option B is a clean alternative.

2. **Metadata for unserviced components**
   - What we know: Component storage and service registry are independent
   - What's unclear: Should `cmd_get_component_metadata` return an error or default values when a digest has no associated service?
   - Recommendation: Return default metadata (all empty/None fields) since the component is valid and uploadable; this matches the success criterion "completes without error for all source types"

## Environment Availability

Step 2.6: SKIPPED (no external dependencies beyond existing Rust/Cargo workspace).

## Sources

### Primary (HIGH confidence)
- `/workspace/packages/wit-schema/src/lib.rs` — full `generate_schema_cached` signature and behavior verified
- `/workspace/packages/wit-schema/src/cache.rs` — `SchemaCache` implementation verified (32-entry LRU, `Mutex<LruCache<ComponentDigest, Value>>`)
- `/workspace/packages/wit-schema/src/types.rs` — `SchemaOptions` struct verified
- `/workspace/app/src-tauri/src/commands.rs` — Tauri command patterns, `AppResult`, `cmd_get_component_digest`, `cmd_publish_component` verified
- `/workspace/app/src-tauri/src/state.rs` — All managed state types verified; `WavsInstanceState.dispatcher()` pattern verified
- `/workspace/app/src-tauri/src/lib.rs` — Registration pattern for commands and managed state verified
- `/workspace/packages/wavs/src/dispatcher.rs` — `Dispatcher` public fields, `engine_manager: EngineManager<S>` verified
- `/workspace/packages/wavs/src/subsystems/engine.rs` — `EngineManager.engine: Arc<WasmEngine<S>>` verified
- `/workspace/packages/wavs/src/subsystems/engine/wasm_engine.rs` — `WasmEngine` struct fields (engine private), available public methods verified
- `/workspace/packages/engine/src/common/base_engine.rs` — `BaseEngine.storage` public, `get_data` available, `wasm_engine` public verified
- `/workspace/packages/types/src/service.rs` — `Component` struct, `ComponentSource` enum, `Permissions` struct all fields verified
- `/workspace/packages/types/src/id/hash.rs` — `ComponentDigest::from_str`, `ComponentDigest::hash` verified
- `/workspace/packages/gui/shared/src/error.rs` — `AppError` variants verified (no `ComponentNotFound` — use `Service(String)`)
- `/workspace/app/src-tauri/Cargo.toml` — wit-schema NOT present (must be added) verified

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all packages verified in codebase
- Architecture: HIGH — all integration points verified in source
- Pitfalls: HIGH — derived from direct code inspection

**Research date:** 2026-04-08
**Valid until:** 2026-05-08 (stable codebase, no external package dependencies)
