# Requirements: WAVS Agent Composition

**Defined:** 2026-04-22
**Core Value:** Developers can write an autonomous LLM agent in ~30 lines of Rust, compile it to WASM, deploy it as a WAVS service, and have it reason + act on triggers with full sandbox and cryptographic trust guarantees.

## v3.0 Requirements

Requirements for agent composition milestone. Each maps to roadmap phases.

### WIT Interface & Types (Foundation)

- [ ] **WIT-01**: `operator.wit` exports a new `run-agent` function returning `result<step-result, string>` where `step-result` is a variant with `done(list<wasm-response>)` and `continue(string)` — backward-compatible with existing `run` export
- [ ] **WIT-02**: `call-service` host import added to operator world — takes service ID + payload bytes, returns result bytes synchronously
- [ ] **WIT-03**: `AllowedServiceCalls` type (All/Only/None) added to `Permissions` in service config with serde default `None`
- [ ] **WIT-04**: `AllowedCallers` type added to service config — callee declares which services may call it (default `None`)
- [ ] **WIT-05**: `max_continuation_steps` field added to component config with default of 10

### Agent Continuation

- [ ] **CONT-01**: Engine re-invocation loop in `run_trigger` — calls `execute_operator_step()`, checks Continue/Done, repeats until Done or max steps
- [ ] **CONT-02**: Auto-persist agent state to KV between steps using `continuation:<service_id>:<correlation_id>:step:N` key pattern — developer can override via opt-out
- [ ] **CONT-03**: Step limit enforcement — engine terminates agent with clear error when `max_continuation_steps` exceeded
- [ ] **CONT-04**: Developer-defined multi-step workflows — named step sequences with explicit `continue("step_name")` handoffs
- [ ] **CONT-05**: Component LRU pinning between continuation steps — compiled module stays cached across re-invocations

### Service-to-Service RPC

- [ ] **RPC-01**: `call-service` host function using `func_wrap_async` — re-entrant `Arc<WasmEngine>` calls `execute_operator_component` directly
- [ ] **RPC-02**: `AllowedServiceCalls` permission enforcement — engine checks caller's permission before dispatching call
- [ ] **RPC-03**: `AllowedCallers` callee-side enforcement — engine checks callee accepts calls from the caller service
- [ ] **RPC-04**: Call depth limit (default 5) with cycle detection — prevents A→B→A deadlocks and unbounded nesting

### Integration & Validation

- [ ] **E2E-04**: Multi-step agent example demonstrating Continue/Done loop with KV-persisted state across steps
- [ ] **E2E-05**: Service composition example — agent calls a utility service via `call-service` and uses the result
- [ ] **E2E-06**: Permission enforcement test — caller without AllowedServiceCalls gets clear error; callee without AllowedCallers rejects call

## Future Requirements

Deferred to v3.x or later milestones.

### Async & Parallel

- **ASYNC-01**: Async message-passing between services (fire-and-forget, result via trigger)
- **ASYNC-02**: Parallel tool execution within agent steps (requires WASI Preview 3 async)

### Advanced Composition

- **COMP-01**: Composable trust-tier calls — call sub-service at on-chain submission tier
- **COMP-02**: Service discovery — components can query available services at runtime

### Observability

- **OBS-01**: Continuation step timeline in Tauri activity feed
- **OBS-02**: Call graph visualization for service-to-service chains

## Out of Scope

| Feature | Reason |
|---------|--------|
| Async service-to-service | WASI Preview 3 async not stable (April 2026); sync-first strategy |
| Parallel tool execution | Single-threaded WASM sandbox; requires ecosystem maturation |
| Agent-to-agent negotiation | Requires higher-level protocol; establish RPC primitive first |
| Streaming continuation | SSE not available in WASI; poll-based continuation is sufficient |
| Cross-node service calls | v3.0 is intra-node; cross-node requires P2P service discovery |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| WIT-01 | Phase 20 | Pending |
| WIT-02 | Phase 20 | Pending |
| WIT-03 | Phase 20 | Pending |
| WIT-04 | Phase 20 | Pending |
| WIT-05 | Phase 20 | Pending |
| CONT-01 | Phase 21 | Pending |
| CONT-02 | Phase 21 | Pending |
| CONT-03 | Phase 21 | Pending |
| CONT-04 | Phase 21 | Pending |
| CONT-05 | Phase 21 | Pending |
| RPC-01 | Phase 22 | Pending |
| RPC-02 | Phase 22 | Pending |
| RPC-03 | Phase 22 | Pending |
| RPC-04 | Phase 22 | Pending |
| E2E-04 | Phase 23 | Pending |
| E2E-05 | Phase 23 | Pending |
| E2E-06 | Phase 23 | Pending |

**Coverage:**
- v3.0 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0

---
*Requirements defined: 2026-04-22*
*Last updated: 2026-04-22 after roadmap creation*
