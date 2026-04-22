# Phase 17: rig-wasi Fork - Context

**Gathered:** 2026-04-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Patch rig-core 0.35.0 to compile cleanly to wasm32-wasip2. This is the compile gate for all downstream agent work — nothing in Phase 18 or 19 is testable until this fork compiles. The fork removes hard WASI blockers: unconditional reqwest, tokio rt feature dependency, cfg inconsistencies, and SSE dead zones.

</domain>

<decisions>
## Implementation Decisions

### Fork Location
- **D-01:** Fork lives in-tree as `packages/rig-wasi`, a workspace member in the WAVS monorepo. No external git dependencies or separate repo.
- **D-02:** Track upstream via `FORK_BASIS.md` in the `packages/rig-wasi/` directory. Document the exact upstream rig-core commit hash (0.35.0 release) and each patch applied. Manual sync when rig releases new versions.

### Patch Scope
- **D-03:** Minimal compile gate only — ~300-500 lines across 6-7 files. Only fix what blocks wasm32-wasip2 compilation. No API changes, no ergonomic cleanup, no module removal.
- **D-04:** Specific patches required:
  1. Make `reqwest` optional behind a feature flag (`Cargo.toml`, `http_client.rs`, `client/mod.rs`)
  2. Make `tokio` optional, replace `tokio::sync::watch` with `futures::channel` equivalent (`Cargo.toml`, `streaming.rs`)
  3. Unify cfg detection to `target_family = "wasm"` everywhere (`wasm_compat.rs`)
  4. Fix SSE module dead zones for wasip2 (`sse.rs`)
  5. Handle `futures-timer` if transitive (uses `std::thread::sleep`)
  6. Verify `getrandom` for wasip2 (remove `wasm_js` feature if present)

### Claude's Discretion
- Exact implementation of the futures::channel replacement for tokio::sync::watch
- Whether to use `cfg(target_family = "wasm")` or introduce a `wasip2` feature flag for detection
- Cargo.toml feature gate naming (e.g., `reqwest` vs `native-http` vs `default`)
- FORK_BASIS.md format and content

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### rig-core WASI investigation
- `/workspace/WAVS_AGENT_IMPROVEMENTS.md` §"Rig WASI Compatibility: Investigation Results" — Detailed blocker analysis, required fork changes table, cfg inconsistency examples
- `/workspace/WAVS_IMPROVEMENTS.md` §"Agent Execution Mode" through §"Agent SDK Crate" — Design context for why this fork exists

### WAVS component patterns
- `examples/components/_helpers/src/prelude.rs` — Standard component imports (Guest, TriggerAction, WasmResponse, host)
- `examples/components/_helpers/src/trigger.rs` — Trigger decode/encode patterns
- `examples/components/echo-data/src/lib.rs` �� Example using `wstd::runtime::block_on` for async
- `examples/components/kv-store/src/lib.rs` — Example using `wasi::keyvalue` host functions

### Research
- `.planning/research/STACK.md` — rig-core 0.35.0 version, specific patches, wstd 0.6.6
- `.planning/research/PITFALLS.md` — Fork divergence risks, cfg flag inconsistencies, block_on constraints

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `example_helpers` crate: provides bindings, prelude, trigger encode/decode — the agent component will use the same pattern
- `wstd` 0.6.5 in workspace (upgrade to 0.6.6 available) — the async runtime for all WASI components
- `export_layer_trigger_world!` macro — component entry point registration

### Established Patterns
- Components implement `Guest` trait with `fn run(TriggerAction) -> Result<Vec<WasmResponse>, String>`
- Async work uses `wstd::runtime::block_on` (see echo-data)
- Host functions accessed via `host::config_var`, `host::log`, `host::get_service`
- KV accessed via `wasi::keyvalue::store::open()` then bucket operations
- All components are `cdylib` crates targeting `wasm32-wasip2`

### Integration Points
- `packages/rig-wasi` will be a new workspace member in `WAVS/Cargo.toml` (rlib, not cdylib)
- Consumer will be `packages/wavs-rig` (Phase 18) which depends on `rig-wasi`
- Final consumer is the example agent component (Phase 19) which depends on `wavs-rig`
- Build target: `wasm32-wasip2` — same as all other example components

</code_context>

<specifics>
## Specific Ideas

No specific requirements — the fork patches are well-scoped in the WAVS_AGENT_IMPROVEMENTS.md investigation. The in-tree approach means we copy rig-core source into packages/rig-wasi and apply patches directly.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 17-rig-wasi-fork*
*Context gathered: 2026-04-20*
