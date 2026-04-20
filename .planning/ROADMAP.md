# Roadmap: WAVS Improvements

## Milestones

- ✅ **v1.0 WAVS Improvements** — Phases 1-6 (shipped 2026-04-07)
- ✅ **v1.1 Open Source AI Providers & Settings UX** — Phases 7-9 (shipped 2026-04-08)
- ✅ **v1.2 Components Explorer** — Phases 10-12 (shipped 2026-04-08)
- ✅ **v1.3 Activity UX & Bug Fixes** — Phases 13-16 (shipped 2026-04-09)
- 🚧 **v2.0 Agent Runtime** — Phases 17-19 (in progress)

## Phases

<details>
<summary>✅ v1.0 WAVS Improvements (Phases 1-6) — SHIPPED 2026-04-07</summary>

- [x] Phase 1: OCI Component Pull (2/2 plans) — completed 2026-03-24
- [x] Phase 2: WIT-to-Schema Tooling (2/2 plans) — completed 2026-03-25
- [x] Phase 3: MCP Execution Interface (3/3 plans) — completed 2026-03-25
- [x] Phase 4: Rust Event Foundation (1/1 plan) — completed 2026-04-07
- [x] Phase 5: Settings Decomposition (2/2 plans) — completed 2026-04-07
- [x] Phase 6: Unified Activity Frontend (2/2 plans) — completed 2026-04-07

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>✅ v1.1 Open Source AI Providers & Settings UX (Phases 7-9) — SHIPPED 2026-04-08</summary>

- [x] Phase 7: Groq & OpenRouter Providers (1/1 plan) — completed 2026-04-08
- [x] Phase 8: Ollama Provider (1/1 plan) — completed 2026-04-08
- [x] Phase 9: Settings Scroll Refactor (1/1 plan) — completed 2026-04-08

Full details: `.planning/milestones/v1.1-ROADMAP.md`

</details>

<details>
<summary>✅ v1.2 Components Explorer (Phases 10-12) — SHIPPED 2026-04-08</summary>

- [x] Phase 10: Backend Commands (1/1 plan) — completed 2026-04-08
- [x] Phase 11: Component Detail Page (2/2 plans) — completed 2026-04-08
- [x] Phase 12: Components List Page (1/1 plan) — completed 2026-04-08

Full details: `.planning/milestones/v1.2-ROADMAP.md`

</details>

<details>
<summary>✅ v1.3 Activity UX & Bug Fixes (Phases 13-16) — SHIPPED 2026-04-09</summary>

- [x] Phase 13: Activity Backend Pipeline (1/1 plan) — completed 2026-04-09
- [x] Phase 14: Activity Frontend UX (1/1 plan) — completed 2026-04-09
- [x] Phase 15: Service Restart Reliability (1/1 plan) — completed 2026-04-09
- [x] Phase 16: Wallet Kebab Menu (1/1 plan) — completed 2026-04-09

Full details: `.planning/milestones/v1.3-ROADMAP.md`

</details>

### v2.0 Agent Runtime (In Progress)

**Milestone Goal:** Make WAVS a first-class agent runtime. Developers write rig-based agents in ~30 lines of Rust that autonomously reason and act inside the WASM sandbox with full cryptographic trust guarantees.

- [ ] **Phase 17: rig-wasi Fork** — Patch rig-core 0.35.0 to compile cleanly to wasm32-wasip2; this is the compile gate for all downstream work
- [ ] **Phase 18: wavs-rig Integration Crate** — Bridge library providing HTTP transport, typed built-in WAVS tools, KV-backed memory, and async entry point shim
- [ ] **Phase 19: Example Agent & E2E Validation** — Full agent loop end-to-end on a live WAVS node with sandboxed LLM access

## Phase Details

### Phase 17: rig-wasi Fork
**Goal**: A patched fork of rig-core 0.35.0 compiles cleanly to wasm32-wasip2, removing all hard WASI blockers: unconditional reqwest, tokio rt feature dependency, cfg inconsistencies across modules, and SSE dead zones
**Depends on**: Nothing (first phase of v2.0)
**Requirements**: FORK-01, FORK-02, FORK-03, FORK-04, FORK-05
**Success Criteria** (what must be TRUE):
  1. `cargo build --target wasm32-wasip2` succeeds on a minimal test component that imports rig-core from the fork with no errors or dead-code warnings from cfg issues
  2. reqwest is optional behind a feature flag and the fork builds without it on wasm32-wasip2 (reqwest not present in the wasm dependency tree)
  3. tokio `rt` feature is absent from the fork; all tokio::sync::watch usages are replaced with futures::channel equivalents that compile on wasm32
  4. `WasmCompatSend`, `WasmBoxedFuture`, and SSE module cfg guards all use `target_family = "wasm"` uniformly — both cfg branches fire correctly with no dead zones
  5. A `FORK_BASIS.md` file in the fork repo pins the exact upstream git rev and documents each patch so divergence is trackable when rig releases updates
**Plans:** 2 plans
Plans:
- [ ] 17-01-PLAN.md — Copy rig-core 0.35.0 source, create fork crate with corrected Cargo.toml feature gates, FORK_BASIS.md
- [ ] 17-02-PLAN.md — Apply source-level patches (reqwest, tokio, cfg, SSE) and verify with wasm32-wasip2 compile probe

### Phase 18: wavs-rig Integration Crate
**Goal**: `packages/wavs-rig` is a library crate that bridges rig into the WASI component sandbox — providing an HTTP transport over wasi:http, five typed built-in tool implementations, KV-backed conversation memory, and the `run_agent` async shim
**Depends on**: Phase 17
**Requirements**: RIG-01, RIG-02, RIG-03, RIG-04, RIG-05
**Success Criteria** (what must be TRUE):
  1. `WasiHttpClient` routes all LLM API calls through `wasi:http/outgoing-handler` implementing rig's `HttpClientExt` trait — a component using it can reach an LLM provider API without any native reqwest
  2. All five built-in tools (KvGetTool, KvSetTool, HttpFetchTool, EvmQueryTool, LogTool) compile to wasm32-wasip2, have typed args/output structs, and produce valid JSON Schema definitions discoverable by rig's tool registry
  3. `WavsMemory` appends messages to KV, retrieves full conversation history, and truncates oldest entries when the conversation exceeds the configured token budget — conversation does not grow unboundedly across invocations
  4. A component implementing `WavsAgent` and calling `run_agent` compiles to wasm32-wasip2 and the full rig agent loop executes correctly inside a single `wstd::runtime::block_on` without nested executor deadlock
  5. A component deployed with `AllowedHostPermission::None` returns a clear human-readable startup error (e.g., "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only") instead of silently trapping
**Plans**: TBD

### Phase 19: Example Agent & E2E Validation
**Goal**: A working example agent component demonstrates the full trigger → LLM reasoning → tool use → structured result loop on a live WAVS node, with `AllowedHostPermission::Only` enforcing that the agent can only reach the configured LLM provider
**Depends on**: Phase 18
**Requirements**: E2E-01, E2E-02, E2E-03
**Success Criteria** (what must be TRUE):
  1. The example agent component contains ~30 lines of domain logic (excluding imports and boilerplate), demonstrating trigger ingestion, LLM reasoning call, at least one tool use, and a typed structured result
  2. A developer can deploy the example using `wavs-mcp` or the CLI, send a trigger, and observe a reasoned structured result returned from the WAVS node with no manual intervention
  3. The example `service.json` uses `AllowedHostPermission::Only(["api.anthropic.com"])` and the agent successfully calls the LLM while the WAVS node blocks any outbound request to a non-listed host
**Plans**: TBD

## Progress

**Execution Order:** 17 → 18 → 19 (strict sequential — each phase is a compile-time prerequisite for the next)

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. OCI Component Pull | v1.0 | 2/2 | Complete | 2026-03-24 |
| 2. WIT-to-Schema Tooling | v1.0 | 2/2 | Complete | 2026-03-25 |
| 3. MCP Execution Interface | v1.0 | 3/3 | Complete | 2026-03-25 |
| 4. Rust Event Foundation | v1.0 | 1/1 | Complete | 2026-04-07 |
| 5. Settings Decomposition | v1.0 | 2/2 | Complete | 2026-04-07 |
| 6. Unified Activity Frontend | v1.0 | 2/2 | Complete | 2026-04-07 |
| 7. Groq & OpenRouter Providers | v1.1 | 1/1 | Complete | 2026-04-08 |
| 8. Ollama Provider | v1.1 | 1/1 | Complete | 2026-04-08 |
| 9. Settings Scroll Refactor | v1.1 | 1/1 | Complete | 2026-04-08 |
| 10. Backend Commands | v1.2 | 1/1 | Complete | 2026-04-08 |
| 11. Component Detail Page | v1.2 | 2/2 | Complete | 2026-04-08 |
| 12. Components List Page | v1.2 | 1/1 | Complete | 2026-04-08 |
| 13. Activity Backend Pipeline | v1.3 | 1/1 | Complete | 2026-04-09 |
| 14. Activity Frontend UX | v1.3 | 1/1 | Complete | 2026-04-09 |
| 15. Service Restart Reliability | v1.3 | 1/1 | Complete | 2026-04-09 |
| 16. Wallet Kebab Menu | v1.3 | 1/1 | Complete | 2026-04-09 |
| 17. rig-wasi Fork | v2.0 | 0/2 | Not started | - |
| 18. wavs-rig Integration Crate | v2.0 | 0/TBD | Not started | - |
| 19. Example Agent & E2E Validation | v2.0 | 0/TBD | Not started | - |
