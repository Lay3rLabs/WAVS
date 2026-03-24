# Phase 2: WIT-to-Schema Tooling - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Developer-facing CLI command (`wavs wit-schema <component.wasm>`) and reusable library that converts compiled WASM component type information into JSON Schema. Covers primitive, record, enum, and variant type mappings, doc comment extraction, and digest-based caching. MCP execution interface (Phase 3) consumes this as a library dependency.

</domain>

<decisions>
## Implementation Decisions

### Variant/Enum Mapping
- **D-01:** WIT variants use externally tagged JSON Schema representation — each variant case is a `oneOf` entry with the case name as a required property key. Matches serde's default Rust enum representation and Wassette's approach. `additionalProperties: false` on each case.
- **D-02:** WIT enums (no-payload, C-style) map to `{"type": "string", "enum": ["case1", "case2"]}` — distinct from variant `oneOf` representation.
- **D-03:** WIT `u128` maps to `{"type": "string", "pattern": "^[0-9]+$"}` with a description noting the underlying type. Standard for blockchain tooling where large integers are common.

### Schema Scope & Structure
- **D-04:** Single JSON output per component containing all exported functions. Top-level structure: `{"world": "...", "exports": {"fn_name": {"inputSchema": {...}, "outputSchema": {...}}}, "$defs": {...}}`.
- **D-05:** Exports only — imported function types (WASI, HTTP, etc.) are not included in the schema. Callers invoke exports; imports are runtime implementation details.
- **D-06:** Shared types deduplicated into `$defs` section with `$ref` pointers. Types used across multiple functions appear once.

### Doc Comment Extraction
- **D-07:** Binary-first strategy — extract docs from Wasmtime's ComponentType API if available. If the binary doesn't expose doc comments, emit schema without descriptions (don't fail). Optional `--wit-path` flag accepts WIT source to enrich schema with doc comments.

### CLI Output & UX
- **D-08:** Always outputs JSON Schema to stdout. No human-readable mode — this is a machine-consumable tool. Diagnostics and warnings go to stderr. Pipe-friendly (works with `jq`, `>`, etc.).

### Claude's Discretion
- Library crate organization — whether to create a separate `packages/wit-schema/` crate or build in CLI first and extract later. Phase 3 needs the logic as a library.
- Exact Wasmtime `ComponentType` API traversal strategy
- Cache storage implementation (in-memory LRU vs persistent disk cache)
- Error message formatting and exit codes
- WIT `result<T, E>` mapping convention (likely special-cased since it's pervasive)
- `option<T>` nullable representation details

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### WIT Interface Definitions
- `wit-definitions/operator/wit/operator.wit` — Operator world definition, `run` export with trigger-action input
- `wit-definitions/aggregator/wit/aggregator.wit` — Aggregator world with 3 exports, complex records and variants
- `wit-definitions/types/wit/core.wit` — Core types including u128, duration, log-level enum
- `wit-definitions/types/wit/chain.wit` — Chain-specific types (EVM, Cosmos)
- `wit-definitions/types/wit/events.wit` — Event types for trigger system
- `wit-definitions/types/wit/service.wit` — Service configuration types

### Existing Code Patterns
- `packages/cli/src/command/exec_component.rs` — CLI command template (component loading, arg parsing, output)
- `packages/engine/src/common/base_engine.rs` — Component loading and LRU caching pattern with ComponentDigest
- `packages/types/src/id/hash.rs` — ComponentDigest type, SHA256 hashing

### Requirements
- `.planning/REQUIREMENTS.md` §WIT-to-Schema — SCHEMA-01 through SCHEMA-05

### External References
- Wassette `component2json` — reference implementation (Bytecode Alliance may upstream per issue #579)
- Wasmtime 42.0.1 component-model API — `Component::component_type()` for type introspection

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ComponentDigest` (`packages/types/src/id/hash.rs`): SHA256 hashing of component bytes — reuse for schema cache key (SCHEMA-05)
- `LruCache<ComponentDigest, T>` pattern (`base_engine.rs`): Mutex-wrapped LRU cache keyed by digest — same pattern for schema cache
- `WasmComponent::new(&engine, &bytes)` (`base_engine.rs`): Component instantiation already working with Wasmtime 42.0.1
- `read_component()` utility in CLI: Reads WASM bytes from file path
- `CliArgs` and `Command` enum in `packages/cli/src/args.rs`: Standard CLI arg pattern

### Established Patterns
- Wasmtime 42.0.1 with `component-model` feature enabled in workspace Cargo.toml
- CLI commands follow `struct + async fn run()` pattern with clap derive
- `--json` flag on existing commands for machine-readable output (though wit-schema is always JSON)
- serde_json for all JSON serialization

### Integration Points
- CLI `Command` enum in `packages/cli/src/args.rs` — add `WitSchema` variant
- `packages/cli/src/command/mod.rs` — add module re-export
- `main.rs` match arm for new command
- Phase 3 MCP server will import the schema generation library

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. User consistently chose recommended defaults aligned with Wassette compatibility and unix conventions.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-wit-to-schema-tooling*
*Context gathered: 2026-03-25*
