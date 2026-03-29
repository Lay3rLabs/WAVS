# Phase 2: WIT-to-Schema Tooling - Research

**Researched:** 2026-03-25
**Domain:** Wasmtime component-model type introspection, JSON Schema generation, WIT doc comment extraction
**Confidence:** HIGH

## Summary

This phase builds a CLI command (`wavs wit-schema <component.wasm>`) and reusable library crate that reads a compiled WASM component binary, introspects its exported function signatures via Wasmtime's `component::types` API, and emits a JSON Schema document describing inputs and outputs. The Wasmtime 42.0.1 API provides complete type introspection through `Component::component_type()` returning a `types::Component`, which exposes `exports(&Engine)` yielding `(&str, ComponentItem)` pairs. For each `ComponentItem::ComponentFunc`, the `params()` and `results()` methods provide full access to parameter names, types (primitives, records, variants, enums, options, results, lists, tuples, flags), enabling recursive schema generation.

Doc comments are NOT embedded in the existing compiled WAVS components (verified: no `package-docs` custom section present in `examples/build/components/*.wasm`). The binary-first strategy (D-07) means we extract type structure from the binary, and optionally enrich with descriptions via `--wit-path` using the `wit-parser` crate (already a transitive dependency).

**Primary recommendation:** Create a `packages/wit-schema/` library crate containing the core type-to-schema conversion logic, then add a thin `WitSchema` CLI command in `packages/cli/` that calls it. This separation enables Phase 3's MCP server to import the library directly.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** WIT variants use externally tagged JSON Schema representation -- each variant case is a `oneOf` entry with the case name as a required property key. Matches serde's default Rust enum representation and Wassette's approach. `additionalProperties: false` on each case.
- **D-02:** WIT enums (no-payload, C-style) map to `{"type": "string", "enum": ["case1", "case2"]}` -- distinct from variant `oneOf` representation.
- **D-03:** WIT `u128` maps to `{"type": "string", "pattern": "^[0-9]+$"}` with a description noting the underlying type. Standard for blockchain tooling where large integers are common.
- **D-04:** Single JSON output per component containing all exported functions. Top-level structure: `{"world": "...", "exports": {"fn_name": {"inputSchema": {...}, "outputSchema": {...}}}, "$defs": {...}}`.
- **D-05:** Exports only -- imported function types (WASI, HTTP, etc.) are not included in the schema. Callers invoke exports; imports are runtime implementation details.
- **D-06:** Shared types deduplicated into `$defs` section with `$ref` pointers. Types used across multiple functions appear once.
- **D-07:** Binary-first strategy -- extract docs from Wasmtime's ComponentType API if available. If the binary doesn't expose doc comments, emit schema without descriptions (don't fail). Optional `--wit-path` flag accepts WIT source to enrich schema with doc comments.
- **D-08:** Always outputs JSON Schema to stdout. No human-readable mode -- this is a machine-consumable tool. Diagnostics and warnings go to stderr. Pipe-friendly (works with `jq`, `>`, etc.).

### Claude's Discretion
- Library crate organization -- whether to create a separate `packages/wit-schema/` crate or build in CLI first and extract later. Phase 3 needs the logic as a library.
- Exact Wasmtime `ComponentType` API traversal strategy
- Cache storage implementation (in-memory LRU vs persistent disk cache)
- Error message formatting and exit codes
- WIT `result<T, E>` mapping convention (likely special-cased since it's pervasive)
- `option<T>` nullable representation details

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SCHEMA-01 | Developer can run `wavs wit-schema <component.wasm>` to generate JSON Schema from a compiled component | Wasmtime 42.0.1 `Component::component_type()` -> `types::Component::exports(&engine)` -> `ComponentItem::ComponentFunc` provides full type introspection. CLI pattern from `exec_component.rs` and `args.rs` shows how to add a new command. |
| SCHEMA-02 | WIT primitive types map to JSON Schema (`u32/u64` -> integer, `string` -> string, `bool` -> boolean, `option<T>` -> nullable) | `types::Type` enum has 13 primitive variants (Bool, S8, U8, S16, U16, S32, U32, S64, U64, Float32, Float64, Char, String). `OptionType::ty()` returns inner type. Mapping table provided below. |
| SCHEMA-03 | WIT record and enum/variant types map to JSON Schema objects and `oneOf` | `Record::fields()` -> `Field { name, ty }`, `Variant::cases()` -> `Case { name, ty: Option<Type> }`, `Enum::names()` -> iterator of case names. D-01/D-02 define exact representation. |
| SCHEMA-04 | WIT doc comments are embedded as JSON Schema `description` fields | Existing binaries have NO `package-docs` section (verified). `wit-parser` crate (transitive dep, v0.230.0+) provides `Function::docs` and `TypeDef` docs via WIT source parsing. D-07 makes this optional via `--wit-path`. |
| SCHEMA-05 | Generated schemas are cached by component SHA256 digest | `ComponentDigest::hash(&bytes)` from `packages/types/src/id/hash.rs` provides SHA256 hashing. `LruCache<ComponentDigest, T>` pattern from `base_engine.rs` provides proven caching pattern. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wasmtime | 42.0.1 | Component type introspection via `component::types` | Already workspace dep, `component-model` feature enabled |
| serde_json | (workspace) | Build JSON Schema as `serde_json::Value` trees | Already workspace dep, no need for a JSON Schema library -- we construct schema programmatically |
| lru | 0.16.1 | In-memory LRU cache for schema by digest | Already workspace dep, proven pattern in `base_engine.rs` |
| clap | (workspace) | CLI argument parsing with derive macros | Already workspace dep, all CLI commands use it |
| sha2 | (workspace) | SHA256 hashing via `ComponentDigest` | Already workspace dep through `wavs_types` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| wit-parser | 0.230.0+ | Parse WIT source files to extract doc comments | Only when `--wit-path` is provided (SCHEMA-04) |
| wasmparser | 0.230.0+ | Parse `package-docs` custom section from WASM binary | Future-proofing for when compiled binaries include doc comments |
| anyhow | (workspace) | Error handling | Standard across all CLI commands |
| tracing | (workspace) | Diagnostic logging to stderr | Warnings and debug info |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual `serde_json::Value` schema construction | `schemars` crate | schemars generates schema FROM Rust types, not FROM WIT types. We need programmatic construction from runtime type introspection. Manual `Value` is correct here. |
| In-memory LRU cache | Persistent disk cache (serde to JSON file) | LRU is simpler, matches existing pattern, sufficient for CLI usage. Phase 3 MCP server will also benefit from in-memory cache since it's long-running. Disk cache adds complexity with no clear benefit. |
| Separate `packages/wit-schema/` crate | Build everything in `packages/cli/` | Separate crate is better because Phase 3 MCP server needs to import schema generation as a library. Building in CLI first would require extraction later. |

**Installation:**
No new dependencies required. All crates are already workspace dependencies. The `wit-parser` crate is a transitive dependency and may need to be added as a direct dependency if doc comment enrichment is implemented.

## Architecture Patterns

### Recommended Project Structure
```
packages/
  wit-schema/
    Cargo.toml
    src/
      lib.rs            # Public API: generate_schema(bytes, options) -> Value
      convert.rs        # WIT Type -> JSON Schema conversion (recursive)
      traverse.rs       # Component type traversal, export function discovery
      cache.rs          # LRU cache keyed by ComponentDigest
      docs.rs           # Doc comment extraction (binary + WIT source)
      types.rs          # Output schema types (WitSchema, ExportSchema)
  cli/
    src/
      command/
        wit_schema.rs   # CLI command handler (thin wrapper)
      args.rs           # WitSchema variant added to Command enum
```

### Pattern 1: Recursive Type-to-Schema Conversion
**What:** A function that pattern-matches on `wasmtime::component::types::Type` and recursively builds `serde_json::Value` JSON Schema objects.
**When to use:** Every type encountered during export function introspection.
**Example:**
```rust
// Verified against wasmtime 42.0.1 docs
fn type_to_schema(ty: &Type, defs: &mut BTreeMap<String, Value>) -> Value {
    match ty {
        Type::Bool => json!({"type": "boolean"}),
        Type::U8 | Type::U16 | Type::U32 => json!({"type": "integer"}),
        Type::U64 | Type::S64 => json!({"type": "integer"}),
        Type::S8 | Type::S16 | Type::S32 => json!({"type": "integer"}),
        Type::Float32 | Type::Float64 => json!({"type": "number"}),
        Type::Char => json!({"type": "string", "maxLength": 1}),
        Type::String => json!({"type": "string"}),
        Type::List(list) => json!({
            "type": "array",
            "items": type_to_schema(&list.ty(), defs)
        }),
        Type::Record(record) => record_to_schema(record, defs),
        Type::Variant(variant) => variant_to_schema(variant, defs),
        Type::Enum(enum_ty) => enum_to_schema(enum_ty),
        Type::Option(opt) => option_to_schema(opt, defs),
        Type::Result(result) => result_to_schema(result, defs),
        Type::Tuple(tuple) => tuple_to_schema(tuple, defs),
        Type::Flags(flags) => flags_to_schema(flags),
        // Resource types, futures, streams -- not expected in WAVS components
        _ => json!({}),
    }
}
```

### Pattern 2: Export Function Discovery with Nested Instance Traversal
**What:** Recursively walk `ComponentItem` exports to find all `ComponentFunc` items, handling nested `ComponentInstance` exports.
**When to use:** When introspecting a compiled component to find all callable exports.
**Example:**
```rust
// Verified: Component::component_type() -> types::Component
// types::Component::exports(&engine) -> impl Iterator<Item = (&str, ComponentItem)>
// ComponentItem variants: ComponentFunc, CoreFunc, Module, Component, ComponentInstance, Type, Resource
fn gather_exports(
    component_type: &types::Component,
    engine: &Engine,
) -> Vec<(String, ComponentFunc)> {
    let mut funcs = Vec::new();
    for (name, item) in component_type.exports(engine) {
        match item {
            ComponentItem::ComponentFunc(func) => {
                funcs.push((name.to_string(), func));
            }
            ComponentItem::ComponentInstance(instance) => {
                // Recurse into instance exports
                for (sub_name, sub_item) in instance.exports(engine) {
                    if let ComponentItem::ComponentFunc(func) = sub_item {
                        funcs.push((format!("{}/{}", name, sub_name), func));
                    }
                }
            }
            _ => {} // Skip non-function exports
        }
    }
    funcs
}
```

### Pattern 3: Schema Deduplication via $defs (D-06)
**What:** Track named types during traversal and emit shared types in a `$defs` section with `$ref` pointers.
**When to use:** When the same record/variant/enum type appears in multiple function signatures.
**Example:**
```rust
// Type deduplication strategy:
// 1. First pass: generate schema inline but track type names
// 2. If a named type is seen more than once, move to $defs
// 3. Replace inline schema with {"$ref": "#/$defs/TypeName"}
//
// Challenge: Wasmtime's Type enum doesn't expose the original WIT type name.
// Strategy: Use record field structure as a "structural fingerprint" to detect
// duplicate types, OR derive names from the WIT source if --wit-path is provided.
// Fallback: Use positional naming like "Record_field1_field2" if no WIT source.
```

### Pattern 4: CLI Command Integration
**What:** Add `WitSchema` variant to the existing `Command` enum in `args.rs`.
**When to use:** Standard pattern for all CLI commands.
**Example:**
```rust
// Following existing pattern from args.rs
#[derive(Parser)]
pub enum Command {
    // ... existing variants ...

    /// Generate JSON Schema from a compiled WASM component
    WitSchema {
        /// Path to the WASI component
        #[clap(long)]
        component: String,

        /// Optional path to WIT source for doc comment enrichment
        #[clap(long)]
        wit_path: Option<PathBuf>,

        #[clap(flatten)]
        args: CliArgs,
    },
}
```

### Anti-Patterns to Avoid
- **Instantiating the component:** Type introspection does NOT require instantiation. `Component::component_type()` works on the uninstantiated component. Do not create a Store, Linker, or Instance.
- **Using Wasmtime's ComponentType trait:** The `ComponentType` trait is for Rust type mapping (e.g., derive macros). We need the `types::Type` enum from `component_type()` for runtime introspection. These are different things with confusingly similar names.
- **Building WIT parser into the hot path:** Doc comment enrichment via `--wit-path` is optional. The core schema generation must work from binaries alone. Keep WIT parsing behind a feature gate or optional codepath.
- **Inlining all types (Wassette approach):** Wassette's `component2json` inlines all types without `$defs`. This works for simple cases but creates massive duplicate schemas for WAVS components (e.g., `trigger-data` variant appears in both operator and aggregator function signatures). D-06 requires deduplication.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SHA256 hashing | Custom hasher | `ComponentDigest::hash()` from `wavs_types` | Already proven, used everywhere in codebase |
| LRU cache | Custom eviction cache | `lru::LruCache` with `Mutex` wrapper | Proven pattern from `base_engine.rs` |
| WIT source parsing | Custom WIT parser | `wit-parser` crate | Complex grammar with packages, interfaces, worlds |
| WASM binary section reading | Manual byte parsing | `wasmparser` crate | Handles all WASM encoding edge cases |
| Component loading | Custom WASM loader | `WasmComponent::new(&engine, &bytes)` | Wasmtime handles validation, parsing |
| File path resolution | Manual path logic | `read_component()` from `cli/src/util.rs` | Handles tilde expansion, workspace paths |

**Key insight:** The entire type introspection capability is provided by Wasmtime's `component::types` module. The novel code in this phase is purely the mapping layer (WIT types -> JSON Schema), not the parsing or introspection.

## Common Pitfalls

### Pitfall 1: ComponentType vs types::Type Confusion
**What goes wrong:** Wasmtime has `wasmtime::component::ComponentType` (a trait for Rust type derivation) AND `wasmtime::component::types::Type` (an enum for runtime introspection). Using the wrong one leads to dead-end compilation errors.
**Why it happens:** Similar names, different purposes. Training data conflates them.
**How to avoid:** Always use `wasmtime::component::types::{Type, ComponentFunc, Record, Variant, ...}` for runtime introspection. Never use the `ComponentType` trait.
**Warning signs:** Import of `wasmtime::component::ComponentType` instead of `wasmtime::component::types::*`.

### Pitfall 2: Engine Requirement for exports()
**What goes wrong:** `types::Component::exports()` and `types::ComponentInstance::exports()` both require an `&Engine` parameter. Forgetting this causes compilation errors.
**Why it happens:** The `Component::component_type()` method takes no engine, but the returned `types::Component`'s methods do.
**How to avoid:** Always keep an `Engine` reference available when traversing types. The engine is created when loading the component anyway.
**Warning signs:** "expected 1 argument, found 0" errors on `.exports()` calls.

### Pitfall 3: WIT Type Names Not Available from Binary
**What goes wrong:** Wasmtime's `types::Type` enum gives you the structural type (fields, cases) but NOT the original WIT type name (e.g., "trigger-action", "wasm-response"). This makes `$defs` naming difficult.
**Why it happens:** The component model binary format encodes type structure but type names are not always preserved in the way you'd expect.
**How to avoid:** Two strategies: (1) Use structural fingerprinting to detect duplicate types and assign generated names, or (2) Use `--wit-path` to resolve names from WIT source. For WAVS components, parameter names from `ComponentFunc::params()` provide hints (e.g., param named "trigger-action" maps to the type).
**Warning signs:** All `$defs` entries have generic names like "type_0", "type_1".

### Pitfall 4: list<u8> Special Case
**What goes wrong:** `list<u8>` in WIT is semantically "bytes" (used for payloads, hashes, etc.) but the generic schema would be `{"type": "array", "items": {"type": "integer"}}`, which is technically correct but useless for JSON callers.
**Why it happens:** JSON doesn't have a native bytes type.
**How to avoid:** Detect `list<u8>` as a special case. Map to `{"type": "string", "contentEncoding": "base64"}` or `{"type": "string", "description": "hex or base64 encoded bytes"}` to match how the MCP layer will serialize/deserialize bytes.
**Warning signs:** Schema says array of integers for what should be a hex string.

### Pitfall 5: result<T, E> Dominates the Output Schema
**What goes wrong:** Nearly every WAVS export function returns `result<T, string>`. Naively mapping this creates a `oneOf` wrapper around every function's output, making the schema harder to use.
**Why it happens:** Error handling is pervasive in WIT.
**How to avoid:** Special-case `result<T, string>` in the output schema: document the `ok` type as the primary `outputSchema` and note the error type in a description or separate field. This matches how callers will consume the result.
**Warning signs:** Every `outputSchema` is a `oneOf` with ok/err branches instead of the actual data type.

### Pitfall 6: Variant vs Enum Confusion
**What goes wrong:** WIT has both `variant` (tagged union with optional payloads) and `enum` (C-style, no payloads). Treating them the same produces incorrect schema.
**Why it happens:** Both are discriminated types, but enum has no payload.
**How to avoid:** Check the Wasmtime type: `Type::Variant` uses `Variant::cases()` -> `Case { name, ty: Option<Type> }`, while `Type::Enum` uses `Enum::names()` -> `&str` iterator. Map per D-01 and D-02.
**Warning signs:** Enum cases have unnecessary `{type: "object"}` wrappers.

## Code Examples

Verified patterns from official Wasmtime 42.0.1 documentation:

### Loading Component and Accessing Type Information
```rust
// Source: https://docs.rs/wasmtime/42.0.1/wasmtime/component/struct.Component.html
use wasmtime::{Config, Engine, component::Component};
use wasmtime::component::types::ComponentItem;

let mut config = Config::new();
config.wasm_component_model(true);
let engine = Engine::new(&config)?;

let wasm_bytes = std::fs::read("component.wasm")?;
let component = Component::new(&engine, &wasm_bytes)?;

// Get type information WITHOUT instantiation
let component_type = component.component_type();

// Iterate exports (requires &engine)
for (name, item) in component_type.exports(&engine) {
    match item {
        ComponentItem::ComponentFunc(func) => {
            // func.params() -> impl Iterator<Item = (&str, Type)>
            // func.results() -> impl Iterator<Item = Type>
            for (param_name, param_type) in func.params() {
                // Process each parameter
            }
        }
        _ => {}
    }
}
```

### Record to JSON Schema (D-01 compatible)
```rust
// Source: https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.Record.html
use wasmtime::component::types::{Record, Field};
use serde_json::{json, Value};

fn record_to_schema(record: &Record, defs: &mut BTreeMap<String, Value>) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for field in record.fields() {
        // field.name: &str, field.ty: Type
        properties.insert(
            field.name.to_string(),
            type_to_schema(&field.ty, defs),
        );
        required.push(json!(field.name));
    }

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}
```

### Variant to JSON Schema (D-01: externally tagged)
```rust
// Source: https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.Variant.html
use wasmtime::component::types::Variant;

fn variant_to_schema(variant: &Variant, defs: &mut BTreeMap<String, Value>) -> Value {
    let mut one_of = Vec::new();

    for case in variant.cases() {
        // case.name: &str, case.ty: Option<Type>
        let case_schema = if let Some(ref payload_ty) = case.ty {
            json!({
                "type": "object",
                "properties": {
                    case.name: type_to_schema(payload_ty, defs)
                },
                "required": [case.name],
                "additionalProperties": false
            })
        } else {
            // No-payload variant case (e.g., "manual" in trigger)
            json!({
                "type": "object",
                "properties": {
                    case.name: { "type": "object", "maxProperties": 0 }
                },
                "required": [case.name],
                "additionalProperties": false
            })
        };
        one_of.push(case_schema);
    }

    json!({"oneOf": one_of})
}
```

### Enum to JSON Schema (D-02)
```rust
// Source: https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.Enum.html

fn enum_to_schema(enum_ty: &wasmtime::component::types::Enum) -> Value {
    let names: Vec<Value> = enum_ty.names()
        .map(|n| json!(n))
        .collect();
    json!({"type": "string", "enum": names})
}
```

### LRU Cache Pattern (from existing codebase)
```rust
// Source: packages/engine/src/common/base_engine.rs
use std::sync::Mutex;
use std::num::NonZeroUsize;
use lru::LruCache;
use wavs_types::ComponentDigest;

pub struct SchemaCache {
    cache: Mutex<LruCache<ComponentDigest, serde_json::Value>>,
}

impl SchemaCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(32).unwrap())
            )),
        }
    }

    pub fn get(&self, digest: &ComponentDigest) -> Option<serde_json::Value> {
        self.cache.lock().unwrap().get(digest).cloned()
    }

    pub fn put(&self, digest: ComponentDigest, schema: serde_json::Value) {
        self.cache.lock().unwrap().put(digest, schema);
    }
}
```

## WIT Type to JSON Schema Mapping Table

Complete mapping for all WIT types encountered in WAVS components:

| WIT Type | JSON Schema | Notes |
|----------|-------------|-------|
| `bool` | `{"type": "boolean"}` | |
| `u8`, `u16`, `u32` | `{"type": "integer", "minimum": 0}` | Add max for precision |
| `s8`, `s16`, `s32` | `{"type": "integer"}` | |
| `u64`, `s64` | `{"type": "integer"}` | JSON numbers are 64-bit float; precision loss possible above 2^53 |
| `float32`, `float64` | `{"type": "number"}` | |
| `char` | `{"type": "string", "maxLength": 1}` | Single unicode codepoint |
| `string` | `{"type": "string"}` | |
| `list<T>` | `{"type": "array", "items": <T-schema>}` | |
| `list<u8>` | `{"type": "string", "contentEncoding": "base64"}` | Special case: bytes |
| `option<T>` | `{"anyOf": [<T-schema>, {"type": "null"}]}` | Nullable |
| `result<T, E>` | `{"oneOf": [{"type":"object","properties":{"ok":<T>},"required":["ok"]}, {"type":"object","properties":{"err":<E>},"required":["err"]}]}` | See Pitfall 5 for output simplification |
| `tuple<T1, T2, ...>` | `{"type": "array", "prefixItems": [<T1>, <T2>, ...], "minItems": N, "maxItems": N}` | Fixed-length |
| `record { ... }` | `{"type": "object", "properties": {...}, "required": [...], "additionalProperties": false}` | D-01 |
| `variant { ... }` | `{"oneOf": [{...}, ...]}` | D-01: externally tagged |
| `enum { ... }` | `{"type": "string", "enum": [...]}` | D-02 |
| `flags { ... }` | `{"type": "array", "items": {"type": "string", "enum": [...]}, "uniqueItems": true}` | Set of flags |
| `u128` (WAVS custom) | `{"type": "string", "pattern": "^[0-9]+$", "description": "128-bit unsigned integer"}` | D-03 |

### WAVS-Specific Type Mapping Notes

The WAVS `u128` record type in WIT is defined as:
```wit
record u128 {
    value: tuple<u64, u64>,
}
```
Per D-03, this should be detected and mapped to the string pattern, NOT to the naive record/tuple schema. Detection strategy: check if a record named "u128" has a single field "value" of type `tuple<u64, u64>`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual WIT parsing for schema | Wasmtime `component::types` runtime introspection | Wasmtime component-model feature stabilized | No need to parse WIT text; work from compiled binary |
| Inline type expansion (Wassette) | `$defs` + `$ref` deduplication (D-06) | Project decision | Smaller schemas for WAVS's complex shared types |
| `package-docs` in binaries | `--wit-path` fallback | Current WAVS components lack docs section | Must support both paths |

**Deprecated/outdated:**
- Wasmtime pre-42 had different `component_type()` signatures; ensure using 42.0.1 docs
- `wit-bindgen` is for generating Rust bindings FROM WIT, not for introspection -- don't confuse with `wit-parser`

## Open Questions

1. **Type Name Recovery from Binaries**
   - What we know: `ComponentFunc::params()` provides parameter names (e.g., "trigger-action"), and structural fingerprinting can detect duplicate types
   - What's unclear: Whether Wasmtime 42's `types::Type` exposes any name hints for records/variants beyond field structure
   - Recommendation: Start with parameter-name-based naming for `$defs`. If the first param is named "trigger-action" and its type is a Record, name the def "trigger-action". For types not directly named by parameters, use structural hashing. Enrich with WIT source names when `--wit-path` is provided.

2. **No-Payload Variant Case Representation**
   - What we know: WIT variants can have cases with no payload (e.g., `manual` in the `trigger` variant). D-01 says externally tagged with `additionalProperties: false`.
   - What's unclear: Should the value be `{}` (empty object), `null`, or `true`?
   - Recommendation: Use `{"type": "object", "maxProperties": 0}` as the property value for no-payload cases, matching a common JSON convention. This way the externally tagged pattern `{"manual": {}}` is valid.

3. **World Name in Schema Output**
   - What we know: D-04 specifies `{"world": "..."}` in top-level schema. `wasm-tools component wit` shows the world name (e.g., "root").
   - What's unclear: The `types::Component` API does not expose a world name directly. The `wasm-tools` output shows "world root" for the echo_data component.
   - Recommendation: Extract from the component's WIT custom section using `wasmparser`, or use a placeholder like the component filename. If `--wit-path` is provided, extract from the parsed WIT.

## Discretion Recommendations

Based on research, these are recommendations for the areas left to Claude's discretion:

### Library Crate Organization
**Recommendation: Create `packages/wit-schema/` as a separate crate from the start.**
Rationale: Phase 3 MCP server needs this as a library dependency. Building in CLI first means extracting later, which is wasted refactoring effort. A thin library crate with `generate_schema(bytes: &[u8], options: SchemaOptions) -> Result<Value>` is clean and reusable.

### Cache Storage
**Recommendation: In-memory LRU cache (same as `base_engine.rs`).**
Rationale: For CLI usage, the cache helps within a single invocation only when processing multiple components. For Phase 3 MCP server (long-running process), the in-memory cache is ideal. Disk cache adds serialization complexity with minimal benefit since schema generation is fast (milliseconds for type introspection, no component instantiation needed).

### result<T, E> Mapping
**Recommendation: For output schemas, unwrap `result<T, string>` to show the `ok` type as primary with error noted in description. For input schemas, show the full `oneOf` if `result` appears.**
Rationale: Every WAVS export returns `result<T, string>`. Wrapping every output in `oneOf` ok/err makes schemas harder to consume. MCP callers care about the success shape. The error is always `string`.

### option<T> Representation
**Recommendation: Use `{"anyOf": [<T-schema>, {"type": "null"}]}`.**
Rationale: This is the standard JSON Schema representation for nullable. Matches serde's default and is widely supported by JSON Schema validators.

### Error Handling and Exit Codes
**Recommendation: Exit 0 on success (schema to stdout), exit 1 on error (message to stderr). Use `anyhow` for error chain. Common errors: file not found, invalid WASM, not a component (core module).**

## Sources

### Primary (HIGH confidence)
- [wasmtime 42.0.1 Component docs](https://docs.rs/wasmtime/42.0.1/wasmtime/component/struct.Component.html) - `component_type()` method, no Engine param
- [wasmtime 42.0.1 types::Type enum](https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/enum.Type.html) - All 26 type variants
- [wasmtime 42.0.1 types::ComponentFunc](https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.ComponentFunc.html) - `params()` returns `(&str, Type)`, `results()` returns `Type`
- [wasmtime 42.0.1 types::Record/Field](https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.Record.html) - `fields()` -> `Field { name, ty }`
- [wasmtime 42.0.1 types::Variant/Case](https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.Variant.html) - `cases()` -> `Case { name, ty: Option<Type> }`
- [wasmtime 42.0.1 types::Enum](https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.Enum.html) - `names()` -> `&str` iterator
- [wasmtime 42.0.1 types::Component](https://docs.rs/wasmtime/42.0.1/wasmtime/component/types/struct.Component.html) - `exports(&engine)` -> `(&str, ComponentItem)`, `imports(&engine)`
- Existing codebase: `packages/engine/src/common/base_engine.rs`, `packages/types/src/id/hash.rs`, `packages/cli/src/args.rs` - Verified patterns
- Existing WIT files: `wit-definitions/operator/wit/operator.wit`, `wit-definitions/aggregator/wit/aggregator.wit`, `wit-definitions/types/wit/core.wit` - Actual WAVS type definitions

### Secondary (MEDIUM confidence)
- [Wassette component2json source](https://github.com/microsoft/wassette/tree/main/crates/component2json) - Reference implementation for WIT-to-JSON-Schema, externally tagged variants using `tag`/`val` pattern
- [wit-parser crate](https://docs.rs/wit-parser/latest/wit_parser/) - `Docs { contents: Option<String> }`, `Function::docs`, `TypeDef` with doc comments
- [wasm-metadata crate](https://docs.rs/wasm-metadata/latest/wasm_metadata/) - General WASM metadata; does NOT provide package-docs extraction

### Tertiary (LOW confidence)
- World name extraction from binary: No verified API found. May require `wasmparser` custom section parsing or `wasm-tools` invocation. Needs validation during implementation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries are already workspace deps, verified versions
- Architecture: HIGH - Wasmtime 42.0.1 API fully documented with verified method signatures
- Type mapping: HIGH - all WIT types in WAVS components catalogued, mapping table derived from Wasmtime API + Wassette reference
- Doc comments: MEDIUM - binary-first approach confirmed (no docs in current binaries), wit-parser exists but integration path not fully verified
- $defs deduplication: MEDIUM - type name recovery from binaries is an open question; structural approach is viable but not verified in production
- Pitfalls: HIGH - verified through API docs and Wassette source comparison

**Research date:** 2026-03-25
**Valid until:** 2026-04-25 (Wasmtime API is stable; WIT type system is mature)
