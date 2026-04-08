# Phase 10: Backend Commands - Context

**Gathered:** 2026-04-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Expose component interface schema and metadata to the frontend via two new Tauri commands. This is the data layer that Phase 11 (Component Detail Page) will consume — no UI work in this phase.

</domain>

<decisions>
## Implementation Decisions

### Command Design
- Two separate Tauri commands: one for JSON Schema (interface/exports), one for metadata (permissions, limits, config)
- Both commands accept a component digest string as input parameter
- Load component bytes from existing component store using digest (reuse existing storage layer)
- Use `generate_schema_cached` from wit-schema for LRU caching of parsed schemas

### Response Shape
- Schema command returns wit-schema's JSON Schema output directly (D-04 spec: `world`, `exports`, `$defs`)
- Metadata command returns a flat struct with typed fields mirroring Component: `permissions`, `fuel_limit`, `time_limit_seconds`, `config`, `env_keys`, `source`
- Source info includes source type + variant-specific fields (URI for Download/OCI, registry for Registry, raw digest for Digest)

### Error Handling & Edge Cases
- Component not found returns typed error with "component not found" message via existing AppResult pattern
- wit-schema parse failure returns error with parse failure details for debugging
- Component with no exports returns valid schema with empty exports array (technically correct)

### Claude's Discretion
No items deferred to Claude's discretion — all areas accepted as recommended.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `wit_schema::generate_schema_cached(engine, component, wasm_bytes, options, cache)` — ready-to-use schema generation with LRU caching
- `SchemaCache` — thread-safe LRU cache (32 default) keyed by `ComponentDigest`
- `Component` struct in `packages/types/src/service.rs` — has `source`, `permissions`, `fuel_limit`, `time_limit_seconds`, `config`, `env_keys`
- `ComponentSource` enum — `Download`, `Registry`, `Digest`, `Oci` variants, each with `.digest()` method

### Established Patterns
- Tauri commands use `#[tauri::command(rename_all = "snake_case")]` decorator with async signatures
- Commands declared in `commands.rs`, imported in `lib.rs`, registered in `tauri::generate_handler![]`
- State injection via `State<'_, StateName>` (e.g., `WavsInstanceState`, `SettingsState`)
- Error handling via `AppResult<T>` type

### Integration Points
- Existing component commands: `cmd_get_component_digest`, `cmd_publish_component`
- Component storage accessible via WavsInstance/engine
- Schema cache should be a new Tauri managed state

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches following existing Tauri command patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
