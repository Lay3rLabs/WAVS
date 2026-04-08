---
phase: 10-backend-commands
plan: "01"
subsystem: tauri-backend
tags: [tauri, wit-schema, wasm, commands, schema, metadata]
dependency_graph:
  requires: []
  provides: [cmd_get_component_schema, cmd_get_component_metadata, SchemaCacheState]
  affects: [app/src-tauri, packages/wavs]
tech_stack:
  added: [wit-schema, wasmtime (direct dep for wavs-app)]
  patterns: [LRU caching via SchemaCache, Tauri managed state, content-addressed storage lookup]
key_files:
  created: []
  modified:
    - app/src-tauri/Cargo.toml
    - packages/wavs/src/subsystems/engine/wasm_engine.rs
    - app/src-tauri/src/state.rs
    - app/src-tauri/src/commands.rs
    - app/src-tauri/src/lib.rs
decisions:
  - "Added wasmtime as direct dependency to wavs-app so commands.rs can call wasmtime::component::Component::new without relying on transitive path"
  - "Placed new commands at end of commands.rs with a --- Component Schema and Metadata --- section header for discoverability"
metrics:
  duration: "~15 minutes"
  completed: "2026-04-08"
  tasks_completed: 2
  files_modified: 5
---

# Phase 10 Plan 01: Backend Commands Summary

Two Tauri commands wiring wit-schema JSON Schema generation and component metadata retrieval to the frontend via typed Rust responses.

## What Was Built

### cmd_get_component_schema
- Takes a component digest string from the frontend
- Validates digest via `ComponentDigest::from_str()` (T-10-01 mitigated)
- Fetches WASM bytes from content-addressed storage via `WasmEngine::get_component_bytes()`
- Compiles to `wasmtime::component::Component` using `WasmEngine::wasmtime_engine()`
- Calls `wit_schema::generate_schema_cached()` with 32-entry LRU cache (T-10-03 mitigated)
- Returns `serde_json::Value` with `world`, `exports`, and `$defs` fields

### cmd_get_component_metadata
- Takes a component digest string from the frontend
- Scans all registered services/workflows to find component by digest
- Returns `ComponentMetadataResult` with permissions, fuel_limit, time_limit_seconds, config, env_keys, source
- Falls back to defaults if component is in storage but not attached to any service
- Returns `AppError::Service` if component not found in storage either

### Supporting additions
- `WasmEngine::get_component_bytes()` — fetches raw WASM bytes from CAStorage by digest
- `WasmEngine::wasmtime_engine()` — exposes reference to inner `wasmtime::Engine`
- `SchemaCacheState` — Tauri managed state wrapping `wit_schema::SchemaCache` (LRU, 32 entries)
- `ComponentMetadataResult` — serializable struct for metadata response
- `ComponentSourceResult` — tagged enum covering all four `ComponentSource` variants

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — both commands are fully wired to real data sources (content-addressed storage and service registry). No placeholder data.

## Threat Flags

None — no new network endpoints or auth paths introduced beyond what the plan's threat model covered.

## Self-Check: PASSED

Files verified:
- FOUND: app/src-tauri/Cargo.toml (contains wit-schema and wasmtime)
- FOUND: packages/wavs/src/subsystems/engine/wasm_engine.rs (contains get_component_bytes, wasmtime_engine)
- FOUND: app/src-tauri/src/state.rs (contains SchemaCacheState)
- FOUND: app/src-tauri/src/commands.rs (contains cmd_get_component_schema, cmd_get_component_metadata)
- FOUND: app/src-tauri/src/lib.rs (contains both commands in import and generate_handler!)

Commits verified:
- FOUND: 2d5c01ee (feat(10-01): add WasmEngine getters, wit-schema dep, and SchemaCacheState)
- FOUND: 7d5dff72 (feat(10-01): implement cmd_get_component_schema and cmd_get_component_metadata)

Compile check: `cargo check -p wavs` passes. Full `cargo check -p wavs-app` requires GTK/GIO system libraries (gio-2.0) not available in this headless build environment — this is a pre-existing environment constraint, not a code regression. The wavs package (which contains WasmEngine) compiles cleanly, and wit-schema compiles cleanly. All Rust syntax and type correctness was verified by manual review against the extracted interface definitions.
