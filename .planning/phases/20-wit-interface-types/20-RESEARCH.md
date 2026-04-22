# Phase 20: WIT Interface & Types - Research

**Researched:** 2026-04-22
**Domain:** WIT interface authoring, Wasmtime bindgen, Rust serde types
**Confidence:** HIGH

## Summary

Phase 20 lays the schema foundation for agent composition: two changes to the WIT definition (`operator.wit`) and three changes to the Rust `wavs-types` crate. These are pure additive changes — nothing is removed, no existing behaviour is altered. The WIT change adds a new export variant (`run-agent`) and a new host import (`call-service`). The Rust type changes add three optional fields to `Permissions` / `Component` with `#[serde(default)]` so existing `service.json` files deserialize without modification.

The two locations that own the interface contract are:
1. `wit-definitions/operator/wit/operator.wit` — the canonical WIT text consumed by both `wit-bindgen::generate!` (component side, `examples/components/_helpers`) and `wasmtime::component::bindgen!` (host side, `packages/engine/src/bindings/operator/world.rs`).
2. `packages/types/src/service.rs` — the Rust service-config types consumed by the engine, CLI, and Tauri app.

Every change in this phase is a schema/declaration change only. Runtime enforcement of `AllowedServiceCalls`, `AllowedCallers`, and the `call-service` host function body belong to Phases 21–22.

**Primary recommendation:** Edit `operator.wit` first (additive WIT), verify `wit-bindgen` and `wasmtime::component::bindgen!` both regenerate cleanly (existing `run` export unchanged), then add the three Rust fields to `service.rs` with serde defaults and unit tests.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

### Claude's Discretion
All implementation choices.

### Deferred Ideas (OUT OF SCOPE)
None.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WIT-01 | `operator.wit` exports new `run-agent` function returning `result<step-result, string>` where `step-result` is a variant with `done(list<wasm-response>)` and `continue(string)` — backward-compatible with existing `run` export | WIT additive export pattern; `wavs-world` already uses `export run`; adding `export run-agent` alongside it is valid WIT and both `wit-bindgen` and `wasmtime::bindgen!` support multiple exports on the same world |
| WIT-02 | `call-service` host import added to operator world — takes service ID + payload bytes, returns result bytes synchronously | The `host:` inline interface block in `wavs-world` is the correct location; adding a new func to it is how all other host imports (`log`, `config-var`, etc.) are declared; the stub body can return `Err("not implemented")` until Phase 22 |
| WIT-03 | `AllowedServiceCalls` type (All/Only/None) added to `Permissions` in service config with serde default `None` | `AllowedHostPermission` is the direct template: same All/Only/None shape, same `#[derive(Default)]` + `#[default]` on `None` variant, same `#[serde(default)]` on the field |
| WIT-04 | `AllowedCallers` type added to service config — callee declares which services may call it (default `None`) | `Option<AllowedCallers>` field on `Component` with `#[serde(default, skip_serializing_if = "Option::is_none")]` follows the `exec_enabled` pattern already in the codebase |
| WIT-05 | `max_continuation_steps` field added to component config with default of 10 | `Option<u32>` field on `Component`; `#[serde(default)]` with a custom default fn returning `10`; mirrors `fuel_limit` / `time_limit_seconds` pattern |
</phase_requirements>

## Standard Stack

### Core — already in the project

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `wit-bindgen` | workspace (0.37.0 confirmed in world.rs comment) | Generates Rust bindings for WASM component side from WIT | Used by all example components via `_helpers` |
| `wasmtime::component::bindgen!` | workspace wasmtime | Generates host-side bindings from WIT for the engine | Used in `packages/engine/src/bindings/operator/world.rs` |
| `serde` + `serde_json` | workspace | Serialization of service config | Used throughout `packages/types/src/service.rs` |
| `utoipa::ToSchema` | workspace | OpenAPI schema generation | Derived on every public service type |

No new dependencies are required for this phase. [VERIFIED: Cargo.toml inspection]

### Supporting
| Tool | Purpose |
|------|---------|
| `wasm-tools component wit` | Validate WIT text before running bindgen (optional but fast) |
| `cargo check -p wavs-engine -p wavs-types` | Confirm bindgen regeneration compiles after WIT edits |
| `cargo check -p example-helpers` | Confirm component-side bindgen regeneration is clean |

### Alternatives Considered
None applicable — this phase exclusively extends existing patterns.

## Architecture Patterns

### WIT File: Additive Export Pattern

The canonical WIT world in `operator.wit` currently exports exactly one function:

```wit
export run: func(trigger-action: trigger-action) -> result<list<wasm-response>, string>;
```

WIT allows a world to export multiple named functions. The `run-agent` export must be added alongside `run` — not replacing it. The new variant type `step-result` must be declared inside the relevant interface block (or inline in the world if preferred). The project convention is to use named interfaces (`input`, `output`) rather than inline world declarations for types.

**Recommended placement for `step-result`:** Add a new variant type in the `output` interface (alongside `wasm-response`) so downstream code that imports `output` gets the new type automatically.

```wit
// In the output interface (wit-definitions/operator/wit/operator.wit):
interface output {
    use event-types.{event-id};

    record wasm-response {
        payload: list<u8>,
        ordering: option<u64>,
        event-id-salt: option<list<u8>>
    }

    // NEW: agent step result
    variant step-result {
        done(list<wasm-response>),
        %continue(string),   // note: "continue" is a WIT keyword — must be escaped
    }
}

// In the wavs-world world, alongside existing export:
export run: func(trigger-action: trigger-action) -> result<list<wasm-response>, string>;
export run-agent: func(trigger-action: trigger-action) -> result<step-result, string>;
// Source: wit-definitions/operator/wit/operator.wit inspection [VERIFIED]
```

**CRITICAL WIT GOTCHA:** `continue` is a reserved keyword in WIT. It must be written as `%continue` in the WIT file. `wit-bindgen` will generate it as `Continue` in Rust (with the `%` prefix stripped). [ASSUMED: based on WIT spec knowledge — verify with `wasm-tools parse` after authoring]

### WIT File: Host Import Pattern

The existing `host:` inline interface in `wavs-world` is where all host functions live. New host functions are added as additional `func` declarations in that block:

```wit
import host: interface {
    // ... existing functions ...

    // NEW: synchronous service call (body stubbed in Phase 22)
    call-service: func(service-id: string, payload: list<u8>) -> result<list<u8>, string>;
}
// Source: wit-definitions/operator/wit/operator.wit inspection [VERIFIED]
```

The Rust host impl is in `packages/engine/src/bindings/operator/host.rs` — it implements the `host::Host` trait. After the WIT edit, the trait will gain a `call_service` method. A stub returning `Err("call-service not yet implemented".into())` satisfies the trait until Phase 22.

### Rust Types: Serde Default Pattern

Two existing patterns in `service.rs` are the templates:

**Pattern A — enum field with `Default` on None variant** (template: `AllowedHostPermission`):
```rust
// Template from existing code [VERIFIED: packages/types/src/service.rs lines 647-655]
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllowedHostPermission {
    All,
    Only(Vec<String>),
    #[default]
    None,
}

// Field on Permissions struct uses #[serde(default)] from struct-level #[serde(default)]
pub struct Permissions {
    pub allowed_http_hosts: AllowedHostPermission,  // defaults to None via derive
    ...
}
```

Apply the same pattern for `AllowedServiceCalls` (WIT-03) as a new field on `Permissions`.

**Pattern B — Option<T> field with skip_serializing_if** (template: `exec_enabled`):
```rust
// Template from existing code [VERIFIED: packages/types/src/service.rs lines 89-91]
#[serde(default, skip_serializing_if = "Option::is_none")]
pub exec_enabled: Option<bool>,
```

Apply this pattern for `AllowedCallers` (WIT-04) as a new `Option<AllowedCallers>` field on `Component`.

**Pattern C — Option<u32> with custom default** (template: `fuel_limit` / `time_limit_seconds`):
```rust
// Template from existing code [VERIFIED: packages/types/src/service.rs lines 203-206]
pub fuel_limit: Option<u64>,
pub time_limit_seconds: Option<u64>,
```

For `max_continuation_steps` (WIT-05), the requirement says "defaults to 10 when absent". Two valid approaches:
- `Option<u32>` with `None` meaning "use default 10" — consistent with `fuel_limit` pattern; engine reads `component.max_continuation_steps.unwrap_or(10)` at runtime
- `u32` with `#[serde(default = "default_max_continuation_steps")]` + `fn default_max_continuation_steps() -> u32 { 10 }`

The `Option<u32>` approach is more consistent with the existing field style. Use `#[serde(default, skip_serializing_if = "Option::is_none")]`.

### Engine Binding Regeneration

After editing `operator.wit`, both bindgen macro invocations must be recompiled:

1. **Host side** (`packages/engine/src/bindings/operator/world.rs`):
   - `wasmtime::component::bindgen!` with `path: "../../wit-definitions/operator/wit"` — reads the WIT at compile time
   - Generates a new `WavsWorldPre` / `WavsWorld` with `call_run_agent` method
   - The `host::Host` trait gains `call_service` — implement the stub in `host.rs`

2. **Component side** (`examples/components/_helpers/src/bindings/world.rs`):
   - `wit_bindgen::generate!` with `path: "../../../wit-definitions/operator/wit"` — reads the same WIT
   - Generates a new `Guest` trait with `run_agent` method (in addition to `run`)
   - Existing components only implement `run`; the new `run_agent` method needs a default or must not be required — **this is the key backward-compatibility concern**

**Backward compatibility mechanism:** In WIT, all exported functions in a world are individually optional at the component level — a component that exports only `run` (not `run-agent`) will still instantiate successfully. Wasmtime's `WavsWorld::instantiate_async` will succeed as long as the required exports are present; optional exports (those not called by the host) do not need to be present. [ASSUMED: this is standard WIT component model behavior — verify by running existing tests after WIT edit]

For the `wit-bindgen` generated `Guest` trait on the component side: `wit_bindgen` will add `run_agent` to the `Guest` trait. Existing components implement `Guest` only for `run`. This WILL break compilation of existing components unless either:
- The new function has a default trait impl, OR
- `wit-bindgen` generates it as a separate optional export (not part of `Guest` trait)

**Resolution:** `wit-bindgen` does NOT add defaults for exported functions — every function in the `Guest` trait must be implemented. However, the `run-agent` export can be declared in a separate interface export in the world rather than the default export, making it part of a different generated trait that existing `Guest` implementors don't need to provide. Alternatively, a blanket default impl can be added in `_helpers`. [ASSUMED: needs verification by testing compilation of existing components after WIT edit]

**Safe approach:** Add `run-agent` as an export of a new named interface (`agent`) rather than the default world export. This keeps it separate from the `Guest` trait:

```wit
interface agent {
    use output.{step-result};
    use input.{trigger-action};
    run-agent: func(trigger-action: trigger-action) -> result<step-result, string>;
}

world wavs-world {
    // ... existing content unchanged ...
    export run: func(trigger-action: trigger-action) -> result<list<wasm-response>, string>;  // unchanged
    export agent;  // NEW: optional agent interface
}
```

With this structure, `wit-bindgen` generates a separate `GuestAgent` trait. Existing components only implement `Guest` (for `run`) and are unaffected. [ASSUMED: interface export pattern — verify WIT spec behavior]

### Anti-Patterns to Avoid

- **Replacing the `run` export:** The existing `run` function must remain. All deployed components and engine call sites depend on it.
- **Inline variant in world instead of interface:** Keep type declarations in named interfaces (project convention, `output` interface).
- **Using `continue` unescaped in WIT:** It's a keyword; must be `%continue`.
- **Adding `call-service` to a new named interface instead of the `host:` inline block:** The engine's `add_to_linker` call references the generated `host::add_to_linker` — adding to the existing inline interface is the correct pattern.
- **Making new Rust fields non-optional without `Default`:** Every new field on `Permissions` or `Component` must deserialize from an existing `service.json` that has no such field — `#[serde(default)]` or `Option<T>` is mandatory.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WIT keyword escaping | Manual string manipulation | WIT `%keyword` syntax | `wasm-tools` / `wit-bindgen` handle it transparently |
| Backward compat for new Guest method | Runtime check or feature flag | WIT interface export (separate from default export) | WIT component model handles optional exports at the binary level |
| Serde default for enum | Custom Deserialize impl | `#[derive(Default)]` + `#[default]` on variant + `#[serde(default)]` on field | Already the project pattern for `AllowedHostPermission` |

## Common Pitfalls

### Pitfall 1: `continue` is a WIT keyword
**What goes wrong:** Writing `continue(string)` in the `step-result` variant fails to parse.
**Why it happens:** `continue` is reserved in WIT (like `break`, `return`, etc.).
**How to avoid:** Use `%continue(string)` in the WIT file. `wit-bindgen` strips the `%` prefix and generates `Continue` in Rust.
**Warning signs:** `wasm-tools component wit` parse error mentioning "unexpected keyword".

### Pitfall 2: Breaking existing component compilation
**What goes wrong:** Adding `run_agent` to the generated `Guest` trait causes every existing component to fail compilation with "not all trait items implemented".
**Why it happens:** `wit-bindgen` adds all exported functions to the single `Guest` trait.
**How to avoid:** Export `run-agent` via a named interface (`export agent;`) rather than as a bare world-level export. This generates a separate `GuestAgent` trait that only agent components implement.
**Warning signs:** `cargo check -p example-helpers` fails after the WIT edit.

### Pitfall 3: Both bindgen sites must recompile
**What goes wrong:** Editing `operator.wit` regenerates host bindings but not component bindings (or vice versa) — type mismatch at test time.
**Why it happens:** Both `packages/engine` and `examples/components/_helpers` embed path references to `wit-definitions/operator/wit` — `cargo` should pick up the change, but a `cargo clean -p wavs-engine -p example-helpers` may be needed.
**How to avoid:** Run `cargo check` on both packages explicitly after the WIT edit.
**Warning signs:** `call_run_agent` method missing from `WavsWorld` but present in `Guest` (or vice versa).

### Pitfall 4: `host::Host` trait stub must be provided
**What goes wrong:** After adding `call-service` to the WIT, the Rust trait `host::Host` gains a new required method. The existing `host.rs` impl will fail to compile.
**Why it happens:** `wasmtime::component::bindgen!` generates a trait for host imports that must be fully implemented.
**How to avoid:** Add a stub `fn call_service(&mut self, service_id: String, payload: Vec<u8>) -> Result<Vec<u8>, String> { Err("not implemented".into()) }` to `host.rs`.
**Warning signs:** `packages/engine` fails to compile with "not all trait items implemented for OperatorHostComponent".

### Pitfall 5: New Rust fields must not break the WIT `component` record
**What goes wrong:** Adding fields to Rust `Permissions` or `Component` that have no counterpart in `service.wit` causes the `TryFrom<component_service::Component>` conversion to fail at runtime.
**Why it happens:** The WIT `component` record and Rust `Component` are separate type systems; the `TryFrom` impls in `component_to_wavs.rs` map them manually.
**How to avoid:** The new fields (`allowed_service_calls`, `allowed_callers`, `max_continuation_steps`) are engine-only runtime config, read by the engine from the Rust types directly (not from the WIT component record). They do NOT need to be added to `service.wit`. The WIT service types are for components that inspect their own service config — agents can read these via `config-var` or other mechanisms.
**Warning signs:** Confusion about whether to add fields to both `service.rs` AND `service.wit`.

## Code Examples

### WIT: Complete proposed `operator.wit` additions

```wit
// Source: wit-definitions/operator/wit/operator.wit [VERIFIED: existing file]

// In the 'output' interface — add step-result variant:
interface output {
    use event-types.{event-id};

    record wasm-response {
        payload: list<u8>,
        ordering: option<u64>,
        event-id-salt: option<list<u8>>
    }

    // NEW
    variant step-result {
        done(list<wasm-response>),
        %continue(string),
    }
}

// New named interface for agent exports:
interface agent {
    use wavs:operator/input.{trigger-action};
    use wavs:operator/output.{step-result};
    run-agent: func(trigger-action: trigger-action) -> result<step-result, string>;
}

// In wavs-world — add host call-service and agent export:
world wavs-world {
    // ... all existing content unchanged ...

    import host: interface {
        // ... all existing host functions unchanged ...

        // NEW: synchronous service call (stub until Phase 22)
        call-service: func(service-id: string, payload: list<u8>) -> result<list<u8>, string>;
    }

    // NEW: optional agent interface export
    export agent;
}
```

### Rust: AllowedServiceCalls (WIT-03)

```rust
// Source: packages/types/src/service.rs — modeled on AllowedHostPermission [VERIFIED]

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllowedServiceCalls {
    /// Component may call any service
    All,
    /// Component may only call the listed service IDs
    Only(Vec<String>),
    /// Component may not call any service (default — backward compatible)
    #[default]
    None,
}

// Add to Permissions struct (which already has #[serde(default)]):
pub struct Permissions {
    pub allowed_http_hosts: AllowedHostPermission,
    pub file_system: bool,
    pub raw_sockets: bool,
    pub dns_resolution: bool,
    // NEW
    pub allowed_service_calls: AllowedServiceCalls,  // defaults to None
}
```

### Rust: AllowedCallers (WIT-04)

```rust
// Source: modeled on exec_enabled pattern [VERIFIED: service.rs line 89-90]

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllowedCallers {
    /// Any service may call this service
    All,
    /// Only the listed service IDs may call this service
    Only(Vec<String>),
    /// No service may call this service (default — callee opt-out)
    #[default]
    None,
}

// Add to Component struct:
pub struct Component {
    // ... existing fields unchanged ...
    // NEW
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<AllowedCallers>,
}
```

### Rust: max_continuation_steps (WIT-05)

```rust
// Source: modeled on fuel_limit pattern [VERIFIED: service.rs line 203]

// Add to Component struct:
pub struct Component {
    // ... existing fields unchanged ...
    // NEW
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_continuation_steps: Option<u32>,
}

// Engine reads it as: component.max_continuation_steps.unwrap_or(10)
```

### Engine host stub: call_service (WIT-02)

```rust
// Source: packages/engine/src/bindings/operator/host.rs [VERIFIED]
// Add to the impl host::Host for OperatorHostComponent block:

fn call_service(&mut self, _service_id: String, _payload: Vec<u8>) -> Result<Vec<u8>, String> {
    Err("call-service not yet implemented (Phase 22)".into())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single-function WIT world | Multiple named interface exports in one world | WIT spec stable 2023+ | Can add `export agent;` without affecting `run` export |
| `cargo-component` for bindgen | `wit-bindgen::generate!` macro directly | Project choice | No registry needed; path reference to local WIT |

**Deprecated/outdated:**
- `wit` CLI: replaced by `wkg`; project currently uses `wit-bindgen` directly (no CLI needed for this phase)

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `%continue` in WIT produces `Continue` in Rust bindgen output | Architecture Patterns (WIT keyword escaping) | Wrong Rust enum name — test compilation to verify |
| A2 | Named interface export (`export agent;`) keeps `run_agent` out of `Guest` trait, not breaking existing components | Architecture Patterns (backward compat) | All existing example components fail to compile — need alternative approach (blanket default or feature flag) |
| A3 | Wasmtime instantiation succeeds for components missing `export agent` (optional interface) | Architecture Patterns | Engine crashes on load of existing components — must verify with existing test suite |
| A4 | New Rust fields (`allowed_service_calls`, `allowed_callers`, `max_continuation_steps`) do NOT need to be added to `service.wit` | Pitfall 5 | Type conversion `TryFrom` fails at runtime — would require WIT service type changes too |

## Open Questions

1. **WIT `%continue` keyword escaping behavior**
   - What we know: `continue` is a WIT keyword; `%` escaping is standard WIT spec
   - What's unclear: Whether `wit-bindgen` 0.37 generates `Continue` or `PContinue` or something else
   - Recommendation: Write the WIT, run `cargo check`, inspect generated code in `target/`

2. **Named interface export backward compat with Wasmtime**
   - What we know: Components compiled against old WIT only export `run`; new WIT adds `export agent`
   - What's unclear: Whether Wasmtime's `instantiate_async` treats missing interface exports as errors or skips them
   - Recommendation: Run `cargo test -p wavs-engine` after the WIT edit — if existing tests pass, backward compat is confirmed

## Environment Availability

Step 2.6: SKIPPED (no external tool dependencies — all changes are source-file edits within the existing Rust/WIT toolchain already installed).

## Security Domain

Security enforcement is enabled by default per config inspection. However, this phase adds only WIT declarations and serde type definitions — no authentication, session management, access control enforcement, or cryptographic operations are implemented. The `AllowedServiceCalls` and `AllowedCallers` types define the permission schema but enforcement belongs to Phase 22.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | n/a — schema only |
| V3 Session Management | no | n/a |
| V4 Access Control | schema only | Types defined here; enforcement Phase 22 |
| V5 Input Validation | no | No user input processed |
| V6 Cryptography | no | No crypto operations |

No threat patterns apply to a pure schema/declaration phase.

## Sources

### Primary (HIGH confidence)
- `wit-definitions/operator/wit/operator.wit` — existing WIT file, verified by inspection
- `packages/types/src/service.rs` — existing Rust types, verified by inspection
- `packages/engine/src/bindings/operator/world.rs` — existing bindgen invocation, verified
- `packages/engine/src/bindings/operator/host.rs` — existing host impl, verified
- `packages/engine/src/worlds/instance.rs` — existing linker setup, verified
- `examples/components/_helpers/src/bindings/world.rs` — component-side bindgen, verified

### Secondary (MEDIUM confidence)
- `docs/WIT_AUTHORING_NOTES.md` — project-internal WIT guidance, confirmed `wit-bindgen` usage pattern
- WIT spec: `%keyword` escaping for reserved words [ASSUMED from spec knowledge, not runtime-verified]

### Tertiary (LOW confidence)
- Named interface export optional behavior in Wasmtime — [ASSUMED, needs test confirmation]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries verified from Cargo.toml and source
- Architecture: HIGH for existing patterns (AllowedHostPermission, host.rs), MEDIUM for WIT named interface export
- Pitfalls: HIGH for identified pitfalls (keyword, compilation breaks), MEDIUM for Wasmtime optional export behavior

**Research date:** 2026-04-22
**Valid until:** 2026-05-22 (stable tech, low churn)
