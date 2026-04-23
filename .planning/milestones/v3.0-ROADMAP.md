# Roadmap: WAVS Improvements

## Milestones

- ✅ **v1.0 WAVS Improvements** — Phases 1-6 (shipped 2026-04-07)
- ✅ **v1.1 Open Source AI Providers & Settings UX** — Phases 7-9 (shipped 2026-04-08)
- ✅ **v1.2 Components Explorer** — Phases 10-12 (shipped 2026-04-08)
- ✅ **v1.3 Activity UX & Bug Fixes** — Phases 13-16 (shipped 2026-04-09)
- ✅ **v2.0 Agent Runtime** — Phases 17-19 (shipped 2026-04-20)
- 📋 **v3.0 Agent Composition** — Phases 20-23 (planned)

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

<details>
<summary>✅ v2.0 Agent Runtime (Phases 17-19) — SHIPPED 2026-04-20</summary>

- [x] Phase 17: rig-wasi Fork (2/2 plans) — completed 2026-04-20
- [x] Phase 18: wavs-rig Integration Crate (3/3 plans) — completed 2026-04-20
- [x] Phase 19: Example Agent & E2E Validation (2/2 plans) — completed 2026-04-20

Full details: `.planning/milestones/v2.0-ROADMAP.md`

</details>

### v3.0 Agent Composition (Planned)

**Milestone Goal:** Agents can reason across multiple invocations and call other deployed services, enabling multi-step autonomous workflows and composable service architectures.

- [x] **Phase 20: WIT Interface & Types** — Establish the `run-agent`/`call-service` interface contract; all engine, SDK, and binding work depends on this compiling first (completed 2026-04-22)
- [x] **Phase 21: Agent Continuation Engine** — Re-invocation loop with KV-backed state persistence, step limit enforcement, and component LRU pinning (completed 2026-04-22)
- [x] **Phase 22: Service-to-Service RPC** — `call-service` host function with permission enforcement, cycle detection, and bilateral caller/callee access control (completed 2026-04-22)
- [x] **Phase 23: Integration & Validation** — End-to-end examples and tests wiring continuation + RPC together, verifying permission enforcement (completed 2026-04-23)

## Phase Details

### Phase 20: WIT Interface & Types
**Goal**: The interface contract for agent composition is locked in — `operator.wit` has the additive `run-agent` export returning `Continue`/`Done` variants, the `call-service` host import is declared, and all new permission/config fields exist in `service.json` types with correct serde defaults
**Depends on**: Phase 19
**Requirements**: WIT-01, WIT-02, WIT-03, WIT-04, WIT-05
**Success Criteria** (what must be TRUE):
  1. A WASM component compiled against the updated `operator.wit` can export both the legacy `run` function and the new `run-agent` function simultaneously — existing components continue to load without modification
  2. The WIT `call-service` host import is declared in the operator world and `wit-bindgen` regenerates bindings without errors — downstream Rust code can reference `call_service()` as a typed function
  3. A `service.json` with `allowed_service_calls: "None"` (or no field at all) deserializes correctly via serde default — existing service configs require zero changes to load on the new runtime
  4. `max_continuation_steps` field appears in the component config schema and defaults to 10 when absent from a service config
  5. `AllowedCallers` field appears in service config with serde default `None` — callee services can declare which callers are permitted without breaking existing configs
**Plans:** 2/2 plans complete
Plans:
- [x] 20-01-PLAN.md — WIT interface: step-result variant, agent export, call-service host import
- [x] 20-02-PLAN.md — Rust service config types: AllowedServiceCalls, AllowedCallers, max_continuation_steps

### Phase 21: Agent Continuation Engine
**Goal**: An agent component returning `Continue` is automatically re-invoked by the engine, with conversation and tool results persisted to KV between steps under the `wavs_agent_step:` key prefix, and a hard step limit that terminates runaway agents with a clear error
**Depends on**: Phase 20
**Requirements**: CONT-01, CONT-02, CONT-03, CONT-04, CONT-05
**Success Criteria** (what must be TRUE):
  1. An agent component that returns `Continue` three times then `Done` is invoked four times total by the engine within a single trigger execution — the final `Done` result is what reaches the aggregator
  2. Between each continuation step, the agent's conversation history and tool results are readable from KV under the `wavs_agent_step:<service_id>:<correlation_id>:step:N` key — a component can resume from exactly where it left off
  3. When an agent exceeds `max_continuation_steps`, the engine terminates it and surfaces a clear error (e.g., `ContinuationLimit: exceeded 10 steps`) — the trigger is not left pending indefinitely
  4. A developer-defined multi-step workflow using named `continue("step_name")` handoffs routes to the correct handler function on each re-invocation — the step name is recoverable from KV state
  5. The compiled WASM module for an active continuation chain is not evicted from the LRU cache between steps — re-instantiation does not occur mid-chain
**Plans:** 2/2 plans complete
Plans:
- [x] 21-01-PLAN.md — Core engine: ContinuationLimit error, agent detection, continuation loop with KV persistence and LRU pinning
- [x] 21-02-PLAN.md — Caller updates and continuation integration tests

### Phase 22: Service-to-Service RPC
**Goal**: An agent or component can synchronously call another deployed service via `call-service`, with both the caller's `AllowedServiceCalls` and the callee's `AllowedCallers` checked before dispatch, cycle detection preventing A->B->A deadlocks, and a depth cap stopping unbounded nesting
**Depends on**: Phase 20
**Requirements**: RPC-01, RPC-02, RPC-03, RPC-04
**Success Criteria** (what must be TRUE):
  1. A component calling `call_service(target_id, payload)` receives the target service's response bytes synchronously within the same trigger execution — no additional trigger event is required
  2. A component with `allowed_service_calls: None` that attempts `call_service()` receives a clear permission error and the call does not reach the target — the caller's `AllowedServiceCalls` is enforced before dispatch
  3. A callee service with `allowed_callers: None` rejects an inbound `call-service` invocation with a clear error — the callee's `AllowedCallers` is enforced independently of the caller's permission
  4. A call chain A -> B -> A is detected and rejected with a cycle error before infinite recursion occurs — the engine tracks the in-flight call stack and refuses to re-enter a service already in the chain
**Plans:** 2/2 plans complete
Plans:
- [x] 22-01-PLAN.md — Engine-side RPC: wasmtime async feature, selective async bindgen, RpcCaller trait, async call_service with permission/cycle checks
- [x] 22-02-PLAN.md — RpcCallerImpl wiring in wavs crate with callee AllowedCallers enforcement, injection into operator execution, RPC tests

### Phase 23: Integration & Validation
**Goal**: The full agent composition surface is exercised end-to-end — a multi-step continuation agent, a service-composition agent that calls a utility service, and a permission enforcement test that proves both `AllowedServiceCalls` and `AllowedCallers` reject unauthorized calls
**Depends on**: Phase 21, Phase 22
**Requirements**: E2E-04, E2E-05, E2E-06
**Success Criteria** (what must be TRUE):
  1. A deployable multi-step agent example exists that triggers, runs 3+ continuation steps with KV-persisted state, and returns a final result — a developer can deploy it and observe each step's KV checkpoint
  2. A deployable service composition example exists where agent A calls utility service B via `call-service` and incorporates B's response into its final result — both services deploy from standard service.json configs
  3. Running a permission enforcement test produces two clear failures: one for a caller missing `AllowedServiceCalls`, one for a callee missing `AllowedCallers` — both rejections include human-readable error messages
**Plans:** 2/2 plans complete
Plans:
- [x] 23-01-PLAN.md — Fix _helpers export macros + multi-step-agent component + continuation E2E test
- [x] 23-02-PLAN.md — Utility-service + composition-agent components + RPC E2E + permission enforcement tests

## Progress

**Execution Order:** 20 -> 21 -> 22 -> 23 (WIT first is non-negotiable; Phase 21 and 22 depend on Phase 20 and can be developed in parallel, but Phase 23 requires both)

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
| 17. rig-wasi Fork | v2.0 | 2/2 | Complete | 2026-04-20 |
| 18. wavs-rig Integration Crate | v2.0 | 3/3 | Complete | 2026-04-20 |
| 19. Example Agent & E2E Validation | v2.0 | 2/2 | Complete | 2026-04-20 |
| 20. WIT Interface & Types | v3.0 | 2/2 | Complete    | 2026-04-22 |
| 21. Agent Continuation Engine | v3.0 | 2/2 | Complete    | 2026-04-22 |
| 22. Service-to-Service RPC | v3.0 | 2/2 | Complete    | 2026-04-22 |
| 23. Integration & Validation | v3.0 | 2/2 | Complete    | 2026-04-23 |
