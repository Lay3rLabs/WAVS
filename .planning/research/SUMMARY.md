# Project Research Summary

**Project:** WAVS v3.0 — Agent Continuation Mode + Service-to-Service RPC
**Domain:** WASM Agent Runtime — multi-step agent execution and synchronous service composition
**Researched:** 2026-04-20
**Confidence:** HIGH — based on direct codebase inspection for all four research areas

## Executive Summary

WAVS v3.0 adds two foundational primitives to an existing, well-architected WASM AVS runtime: agent continuation mode (multi-step agents that return `Continue`/`Done` variants instead of a flat result) and synchronous service-to-service RPC via a `call-service` host function. Both features extend the existing execution model rather than replacing it — the Dispatcher, Aggregator, Submission, and TriggerManager subsystems are untouched. All changes are scoped to the WIT interface layer (`operator.wit`), the engine's re-invocation loop, the host component's capability bindings, and the `wavs-rig` guest SDK. No new external crates are required; every mechanism builds on primitives already present in the workspace.

The recommended approach follows patterns established by Cloudflare Workflows, LangGraph, and Temporal: KV-backed state persistence (not inline state in the WIT return value), explicit step count limits, default-deny permission allowlists, and synchronous RPC with per-call depth tracking. WAVS's differentiator over all of these systems is applying these patterns inside a cryptographically-signed, sandboxed Wasmtime runtime with EVM/Cosmos integration — each step's result can be aggregated and submitted on-chain. The implementation surface is deliberately small: roughly 200 lines of net-new Rust, one WIT variant addition, one new host function, and one new SDK return type.

The primary risks are: (1) the Tokio deadlock trap for `call-service` if the host function is not implemented using `func_wrap_async`; (2) multi-operator consensus stalls if LLM calls inside continuation steps use temperature > 0; and (3) continuation state exceeding the existing 4 KB payload cap if state is passed inline rather than through KV. All three are design-time decisions that must be locked in during Phase 1 — they cannot be patched after the interface is finalized.

## Key Findings

### Recommended Stack

Zero new external crates are required. v3.0 is built entirely from existing workspace primitives: `wasmtime 42.0.1` (already configured with `async_support(true)`, enabling `func_wrap_async` for the `call-service` host function); `wasi:keyvalue 0.2.0-draft2` (existing host capability, used for KV-backed continuation state persistence); `wit-bindgen 0.53.1` (regenerated after WIT change, no version bump); `serde`/`serde_json` (serialization of `AgentContinuation` state blob); and `thiserror` (two new `EngineError` variants). The existing `AllowedHostPermission` pattern in `Permissions`/`service.json` is mirrored exactly for the new `AllowedServiceCalls` field.

**Core technologies:**
- `wasmtime 42.0.1` (pinned): WASM execution and async host functions — `func_wrap_async` required for `call-service` to avoid Tokio deadlock
- `wasi:keyvalue 0.2.0-draft2`: Continuation state persistence under `wavs_agent_step:` key prefix (distinct from existing `wavs_agent_memory:` prefix)
- `wit-bindgen 0.53.1`: WIT codegen after `operator.wit` variant change — no tooling version change
- `wavs_types::Permissions` + `AllowedServiceCalls` enum: mirrors existing `AllowedHostPermission` pattern exactly; serde default `None` preserves backward compatibility
- `tokio` workspace: async host function body; step loop; `.await` on recursive `execute_operator_component`

### Expected Features

**Must have (table stakes) — v3.0:**
- `Continue`/`Done` WIT return variant on `operator.wit`'s `run` export — all continuation depends on this
- Engine re-invocation loop in `run_trigger` with KV-backed auto-persist of continuation state
- `max_continuation_steps` enforcement (default 10) — safety invariant; must ship with continuation
- `call-service` synchronous RPC host function — the composability primitive
- `AllowedServiceCalls` in `service.json` + engine enforcement — default-deny; ships with `call-service`
- Engine inter-service dispatch reusing `execute_operator_component` — no new execution machinery

**Should have (competitive differentiators):**
- Cryptographically signed multi-step results (inherits from existing WAVS signing pipeline — zero new code)
- Per-component sandbox enforcement across the call graph (each callee runs with its own permissions — structural)
- Agent-decided vs. developer-defined workflows in same API (distinction is inside the component; engine is agnostic)
- KV state continuity without developer boilerplate (engine manages checkpoint; developer writes no checkpoint code)

**Defer to v3.x:**
- `call-service` with trust tier parameter (result-only / signed / on-chain) — high value for DeFi but HIGH complexity
- `read_continuation_state()` ergonomic helper in `wavs-rig`
- Activity feed multi-step trace UI (step N of M, state at each step)
- `AllowedCallers` callee-side enforcement (bilateral permission model)
- `call-service` call depth enforcement as a configurable `max_depth` field

**Defer to v4+:**
- Async parallel service calls (requires WASI 0.3 / Preview 3 — not stable as of 2026)
- Shared KV namespaces between services
- Service graph visualizer in Tauri app

### Architecture Approach

Both features integrate *into* the existing five-subsystem architecture (Trigger → Dispatcher → Engine → Aggregator/Submission) without touching any subsystem except Engine. The continuation loop lives inside a single `ctx.rt.spawn` task — the Dispatcher sees only the final `Done` result as a normal `EngineResponse::Operator`. The `call-service` host function uses a re-entrant `Arc<WasmEngine>` call directly within the same async task — routing through the Dispatcher would deadlock because `EngineManager::start()` is a blocking `while let Ok(command) = rx.recv()` loop. Both features require WIT changes to `operator.wit` first, after which engine and SDK work can proceed.

**Major components changed:**
1. `wit-definitions/operator/wit/operator.wit` — new `agent-step-result` variant on `run` export; new `call-service` in host interface
2. `packages/engine/src/worlds/operator/` — re-invocation loop (`execute_operator_step`), `call-service` host function implementation, `OperatorHostComponent` gains `allowed_service_calls`, `Arc<WasmEngine>`, call depth counter
3. `packages/types/src/service.rs` — `Permissions` gains `allowed_service_calls: AllowedServiceCalls`; `Component`/`Workflow` gains `max_continuation_steps`
4. `packages/wavs-rig/src/agent.rs` — `WavsAgent` trait return type widens to `AgentOutput<T>` enum (`Done`/`Continue`)
5. `packages/engine/src/bindings/` — regenerated WIT bindings (no manual changes)

**Components untouched:** Dispatcher, TriggerManager, SubmissionManager, Aggregator, P2P layer.

### Critical Pitfalls

1. **`call-service` must use `func_wrap_async`, not `func_wrap` + `block_on`** — sync host function blocks the Tokio worker thread; under any concurrent load this deadlocks the entire node. Recovery requires a full node restart. `Config::async_support(true)` is already set; `func_wrap_async` is the idiomatic path.

2. **Continuation state must be KV-backed (key-only in WIT return), not inline** — the existing 4 KB payload cap (`max_wasm_payload_size`) applies to `WasmResponse` payloads. An agent accumulating conversation history reaches this limit in 2-3 steps. The `Continue` WIT return value must carry only the KV key (< 64 bytes); full state lives in `wasi:keyvalue` under `wavs_agent_step:<correlation_id>:<step>`.

3. **Multi-operator LLM calls require temperature=0** — non-deterministic LLM responses produce different `Continue` state blobs across operators; the `QuorumQueue` keys by `(EventId, SubmitAction)` and never reaches consensus. This is an architectural constraint on all continuation agents deployed in multi-operator mode.

4. **`call-service` must not route through the Dispatcher** — the re-entrant `Arc<WasmEngine>::execute_operator_component()` call within the host function is the correct pattern. A new `EngineCommand::CallService` channel message would deadlock: the blocking `rx.recv()` loop in `EngineManager::start()` cannot respond while the engine task is mid-execution.

5. **WIT interface change is breaking for legacy components** — changing `run`'s return type from `result<list<wasm-response>, string>` to `result<agent-step-result, string>` breaks all components compiled against `wavs:operator@2.7.0`. The versioning strategy (dual linker fallback OR separate `call-run-continuation` export) must be decided before the interface is published.

6. **Cycle detection required from day one** — mutual `AllowedServiceCalls` between service A and B creates an infinite call loop. Engine must track the in-flight call chain and reject any target already in the stack. No step limit prevents A→B→A cycles because each service resets its own counter.

## Implications for Roadmap

Based on dependencies identified across all four research files, a four-phase build order is strongly indicated.

### Phase 1: WIT Interface + Types Foundation
**Rationale:** Every downstream change (engine loop, SDK, bindings, host function) depends on the WIT types compiling. This phase establishes the interface contract with no behavior change — it is a pure types-and-schema phase that lets all subsequent work proceed in parallel or sequence.
**Delivers:** New `operator.wit` with `agent-step-result` variant and `call-service` host import; new `AllowedServiceCalls` enum and `allowed_service_calls` field in `Permissions`; `max_continuation_steps` on `Component`/`Workflow`; regenerated bindings in `packages/engine/src/bindings/`. Decision on WIT backward-compatibility strategy (dual linker vs. additive export) must be made here.
**Addresses features:** `Continue`/`Done` WIT variants (P1 blocker); `AllowedServiceCalls` schema; `max_continuation_steps` schema
**Avoids pitfalls:** WIT versioning break (Pitfall 9); continuation state size cap (Pitfall 5 — KV-key-only return type decided here)

### Phase 2: Agent Continuation Engine Loop
**Rationale:** Continuation mode has no dependency on `call-service`. It can be built and tested in isolation once Phase 1 types compile. Testing is straightforward: write a component that returns `Continue` N times then `Done`, verify step limit enforcement, verify KV persistence, verify fresh store per step.
**Delivers:** `execute_operator_step()` single-step method on `WasmEngine`; `run_trigger_with_continuation()` loop in `EngineManager`; KV auto-persist under `wavs_agent_step:` prefix; step limit enforcement (`EngineError::ContinuationLimit`); component `Arc` pinning per active chain (prevents LRU eviction between steps); updated `WavsAgent` trait in `wavs-rig`.
**Addresses features:** Engine re-invocation loop; auto-persist continuation state; max-step enforcement; developer-defined step sequencing (convention inside component)
**Avoids pitfalls:** Runaway agent loops (Pitfall 6); LRU cache eviction between steps (Pitfall 7); re-instantiation model misunderstanding (Pitfall 1); KV inline state size (Pitfall 5)

### Phase 3: Service-to-Service RPC + Permission Enforcement
**Rationale:** Depends on Phase 1 types but not Phase 2 continuation — can be developed in parallel with Phase 2 if staffing allows, but Phase 1 must complete first. The `call-service` host function is the most complex and highest-risk item; it must use `func_wrap_async` and include cycle detection from the first implementation.
**Delivers:** `call-service` host function registered via `func_wrap_async` (not `func_wrap`); `OperatorHostComponent` gains `Arc<WasmEngine<S>>`, `Services`, call depth counter; `AllowedServiceCalls` permission check (caller-side); call chain cycle detection (reject A→B→A); `call_service()` binding in `wavs-rig`; updated `InstanceDepsBuilder` to pass engine and services refs.
**Addresses features:** `call-service` synchronous RPC; `AllowedServiceCalls` enforcement; engine inter-service dispatch via existing `execute_operator_component`
**Avoids pitfalls:** Tokio deadlock (Pitfall 3 — `func_wrap_async` mandatory); circular dependency loops (Pitfall 8); Dispatcher routing deadlock anti-pattern

### Phase 4: Integration, Validation, and E2E Tests
**Rationale:** Wire both features together, validate multi-operator consensus, confirm backward compatibility with legacy components, and add observability hooks.
**Delivers:** E2E test: agent A triggers, calls service B, returns combined result; backward compatibility test (legacy `@2.7.0` component loads on new engine via fallback path); multi-operator 2-node test with temperature=0 LLM calls reaching quorum; activity feed `ContinuationStep` events for operator visibility; service.json schema documentation for `AllowedServiceCalls` and `max_continuation_steps`.
**Addresses features:** Integration of continuation + RPC; developer experience; operator observability
**Avoids pitfalls:** Multi-operator consensus stall (Pitfall 2 — validated here); KV isolation misunderstanding (Pitfall 4 — documented and tested)

### Phase Ordering Rationale

- **WIT first** is non-negotiable: all Rust bindings are generated from WIT; no engine or SDK work compiles against the new interface until WIT change and bindgen pass.
- **Continuation before RPC integration testing** (but Phase 3 can run in parallel with Phase 2 at the implementation level): continuation is self-contained and testable in isolation; RPC requires both WIT + the re-entrant engine pattern, but not the continuation loop.
- **Security-first within each phase**: step limits ship with continuation (Phase 2); cycle detection ships with `call-service` (Phase 3). No feature ships without its paired safety guard.
- **No Dispatcher changes in any phase**: the crossbeam channel architecture is stable and correct; re-entrant `Arc<WasmEngine>` is the deliberate alternative to new channel messages.

### Research Flags

Phases needing deeper research during planning:
- **Phase 3 (`call-service` host function):** The re-entrant `Arc<WasmEngine>` pattern involves subtle Wasmtime store lifetimes. Before implementation, verify that `execute_operator_component` can be called re-entrantly within the same Tokio task without Store aliasing violations. Wasmtime issue #9600 flags this as requiring careful store management.
- **Phase 4 (multi-operator consensus at temperature=0):** Validate that the specific LLM provider(s) used in agent services produce byte-identical outputs at temperature=0 across different operator machines. Model provider behavior varies; this needs an empirical test before shipping multi-operator agents.

Phases with standard, well-documented patterns (skip research-phase):
- **Phase 1 (WIT + types):** WIT variant types and serde-default field additions are standard. The `AllowedServiceCalls`/`AllowedHostPermission` mirror pattern is already in the codebase.
- **Phase 2 (continuation loop):** The re-invocation loop is ~100 lines of Rust following patterns already in `run_trigger`; KV key-prefix convention follows `wavs_agent_memory:` precedent.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All primitives verified by direct codebase inspection; zero new crates required |
| Features | MEDIUM-HIGH | Table stakes confirmed by LangGraph/Temporal/Cloudflare analogues; WAVS-specific implementation paths drawn from codebase; differentiators are structural (inherit from existing WAVS) |
| Architecture | HIGH | All subsystem boundaries verified by direct inspection of dispatcher.rs, engine.rs, execute.rs, instance.rs, wasm_engine.rs |
| Pitfalls | HIGH | Critical pitfalls verified against actual code: `func_wrap` API, `max_wasm_payload_size`, `QuorumQueue` keying by `(EventId, SubmitAction)`, KV per-service namespace construction |

**Overall confidence:** HIGH

### Gaps to Address

- **WIT backward-compatibility strategy:** Two viable paths (dual linker at `@3.0.0` vs. additive `call-run-continuation` export). Recommendation: additive `call-run-continuation` export (lower risk, no dual-linker complexity). Must be decided and documented in Phase 1 before WIT is published.
- **`AllowedCallers` callee-side enforcement:** Research flags this as a v3.x item but it is a meaningful security gap in multi-tenant deployments. Phase 4 planning should include a go/no-go decision on including it in v3.0.
- **Fuel budget guidance for agent services:** Per-step fuel limits are sized for simple query components. Agent continuation steps require 10-50x more fuel. Default configuration and documentation gap; address in Phase 2 alongside step limit implementation.
- **LRU cache pin implementation detail:** The component-pinning mitigation (hold `Arc<Component>` per active chain) needs validation against the actual `WasmEngine` LRU cache structure before Phase 2 begins.

## Sources

### Primary (HIGH confidence)
- Direct inspection: `packages/wavs/src/dispatcher.rs`, `packages/wavs/src/subsystems/engine.rs`, `packages/engine/src/worlds/operator/execute.rs`, `packages/engine/src/worlds/operator/component.rs`, `packages/engine/src/worlds/instance.rs`, `packages/wavs/src/subsystems/engine/wasm_engine.rs`
- Direct inspection: `wit-definitions/operator/wit/operator.wit`
- Direct inspection: `packages/types/src/service.rs`, `packages/engine/src/backend/wasi_keyvalue/context.rs`
- Direct inspection: `packages/wavs-rig/src/agent.rs`, `packages/wavs-rig/src/memory.rs`
- `.planning/PROJECT.md` — v3.0 scope and requirements
- [Wasmtime `func_wrap_async` docs](https://docs.wasmtime.dev/api/wasmtime/component/struct.LinkerInstance.html)
- [WIT variant spec](https://component-model.bytecodealliance.org/design/wit.html)

### Secondary (MEDIUM confidence)
- [Cloudflare Workflows GA blog](https://blog.cloudflare.com/workflows-ga-production-ready-durable-execution/) — step-based auto-persist model
- [LangGraph recursion_limit docs](https://python.langchain.com/v0.1/docs/modules/agents/how_to/max_iterations/) — step limit as table stakes
- [wasmCloud RPC docs](https://wasmcloud.com/docs/hosts/lattice-protocols/rpc/) — actor-to-actor call patterns
- [NVIDIA sandboxing guidance](https://developer.nvidia.com/blog/practical-security-guidance-for-sandboxing-agentic-workflows-and-managing-execution-risk/) — default-deny + allowlist baseline
- [Wasmtime issue #9600](https://github.com/bytecodealliance/wasmtime/issues/9600) — re-entrant WASM component calls

### Tertiary (LOW confidence)
- [Zylos Research AI Agent Checkpointing](https://zylos.ai/research/2026-03-04-ai-agent-workflow-checkpointing-resumability) — single source; consistent with Cloudflare/Temporal patterns
- [WASM Component Model async timeline](https://github.com/WebAssembly/component-model/issues/316) — confirms WASI 0.3 async not stable; justifies deferring parallel service calls

---
*Research completed: 2026-04-20*
*Ready for roadmap: yes*
