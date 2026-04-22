# Feature Research

**Domain:** WASM Agent Runtime — Agent Continuation Mode + Service-to-Service RPC (WAVS v3.0)
**Researched:** 2026-04-20
**Confidence:** MEDIUM-HIGH (patterns from Cloudflare Workflows, LangGraph, wasmCloud wRPC, and Temporal informed the analysis; WAVS-specific implementation paths drawn from reading the actual codebase; source links below)

## Feature Landscape

### Table Stakes (Users Expect These)

Features developers expect from any multi-step agent or composable service runtime. Missing these = the system feels like a prototype, not a platform.

| Feature | Why Expected | Complexity | WAVS Dependency | Notes |
|---------|--------------|------------|-----------------|-------|
| Multi-step agent loop (Continue/Done) | Every real-world agent workflow has more than one reasoning step — research, plan, execute, verify. A single-invocation loop is not enough for non-trivial tasks. LangGraph, Temporal, Cloudflare Workflows, and OpenAI Agents SDK all treat continuation as the default model. | HIGH | Engine re-invocation loop; KV-backed WavsMemory (existing) | New `AgentStep` WIT return type with `Continue { state }` and `Done { result }` variants. Engine detects `Continue`, persists state to KV, re-invokes the same component with the continuation payload. A max-step limit is mandatory to prevent infinite loops — this is table stakes in every framework (LangGraph `recursion_limit`, Temporal workflow timeouts). |
| Auto-persist state between steps | If the developer has to manually checkpoint to KV on every Continue, they will forget it and lose state on crashes. The runtime should handle this automatically. Cloudflare Workflows persists step results automatically; LangGraph checkpoints every node by default. | MEDIUM | Existing `wasi:keyvalue` host; existing `WavsMemory` (conversation history) | On `Continue`, engine serializes the continuation payload + current conversation snapshot to a well-known KV key (keyed by service+event). On next invocation, the component reads from that key. Developer can override by writing to KV directly before returning `Continue`. |
| Synchronous service-to-service call | Agents calling other deployed services is the baseline for composition. wasmCloud, Spin, and Cloudflare Workers all provide synchronous component-to-component call as a first-class primitive. Without it, the only composition model is trigger chaining (fire-and-forget), which is too loose for most real use cases. | HIGH | Engine inter-service dispatch; service registry (existing); existing `execute_operator_component` | New `call-service` host function exposed in the WIT world. Component calls it synchronously (blocks inside WASM), engine dispatches to the target service's component and returns the result. Target executes in the same process (no network hop for local calls). |
| Permission-based service calling | Developers expect that a deployed service cannot arbitrarily call any other service. Permission prompts and allowlists are industry standard for agent security. OpenAI Codex agent approvals, Cloudflare Workers bindings, and NVIDIA's sandboxing guidance all treat default-deny + explicit allowlist as the baseline. | MEDIUM | Existing `AllowedHostPermission` pattern in service.json and engine | New `AllowedServiceCalls` field in `service.json` (mirrors existing `AllowedHostPermission`). Engine checks caller's allowlist before dispatching `call-service`. Attempting to call an unlisted service returns an error, not a panic. |
| Developer-controlled step sequencing | Developers who know their workflow in advance should be able to express it as a deterministic sequence (not LLM-decided). This is the "script" mode vs. "autonomous" mode. Cloudflare Workflows and Temporal both distinguish between deterministic steps and agent-decided steps. | MEDIUM | Agent continuation loop (above) | In `run()`, developer returns `Continue { next_step: "step_name", state }` with explicit step routing. The re-invoked component reads the step name from state and dispatches to the right handler. No new engine machinery needed — it is a convention inside the component. Agent-decided mode: LLM picks the next action; developer-defined mode: Rust match on step name. |
| Step count and fuel limits | Without hard limits on continuation steps, a buggy or adversarial agent burns operator resources indefinitely. Every production agent framework has this: LangGraph `recursion_limit`, Temporal workflow timeouts, Cloudflare Workflows step limits. | LOW | Existing Wasmtime fuel/timeout limits (per-invocation) | Add a `max_continuation_steps` field to `service.json` (default: sensible cap like 10). Engine tracks invocation count across continuation steps for the same event and hard-stops at the limit, returning an error to the caller. Per-step fuel limits already exist in WAVS and apply to each re-invocation. |

### Differentiators (Competitive Advantage)

Features that make WAVS's agent composition meaningfully different from other frameworks.

| Feature | Value Proposition | Complexity | WAVS Dependency | Notes |
|---------|-------------------|------------|-----------------|-------|
| Cryptographically signed multi-step results | Every step result — intermediate and final — is signed by operators. A 5-step research agent produces a chain of signed outputs, not just a final answer. No other agent framework provides this. | LOW | Existing operator signing (inherits from WAVS) | Agent continuation results route through the existing aggregator and signing pipeline. No new code at the signing layer — the engine just re-enters the existing pipeline with the continuation payload as the new trigger input. |
| Sandbox enforcement across the call graph | When service A calls service B via `call-service`, service B runs with its own `AllowedHostPermission` and `AllowedServiceCalls` enforcement — not the caller's permissions. Privilege escalation via composition is impossible by construction. wasmCloud achieves this via wRPC capability providers; WAVS achieves it by re-running the target component in its own Wasmtime instance. | MEDIUM | Existing per-component sandbox model | Each `call-service` invocation spins up the target component in a fresh `InstanceDeps` with the target service's permissions. The caller's permission scope does not bleed into the callee. This is structural and is a strong differentiator vs. native multi-agent frameworks where all agents share the same process memory. |
| Agent-decided vs. developer-defined workflows in the same API | The LLM decides when to continue (autonomous mode) or the developer hard-codes the sequence (scripted mode), and both use the same `Continue`/`Done` return type. The developer picks the model that fits their use case without switching frameworks. Most systems force a choice: Temporal is deterministic-only; open-ended LLM agents are autonomous-only. | LOW | Agent continuation (above) | The distinction is entirely inside the component: `Continue { state: llm_next_action }` vs. `Continue { state: "step_2" }`. The engine does not know or care. |
| Composable service graph with per-node trust tiers | A caller can invoke a target service at any of the three WAVS trust tiers (result only / signed result / on-chain submission). Composition is not just "call and get a result" — it is "call and get a cryptographically verified result that I can submit on-chain." No agent framework currently offers this. | HIGH | Existing three trust tiers in wavs-mcp and engine | `call-service` accepts a trust tier parameter. For on-chain submission tier, the engine's normal submission path fires for the sub-call. This adds significant complexity but is the feature that makes WAVS agent composition relevant to DeFi and verifiable AI use cases. Defer to v3.x if too complex for initial shipping. |
| KV state continuity without developer boilerplate | Auto-persisted continuation state means a component can crash mid-execution, be restarted, and resume from the last checkpoint — without the developer writing a single line of checkpoint code. Cloudflare Workflows and Temporal provide this; no Rust WASM framework does. | MEDIUM | Existing `wasi:keyvalue` and `WavsMemory` | Engine writes continuation state to `wavs_continuation:{service_id}:{event_id}` in KV before returning from the current invocation. On re-invocation, the component reads from that key via the existing `KvGetTool` or the new `read_continuation_state()` helper in `wavs-rig`. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Async / parallel service calls | Looks like a performance win — call 3 services in parallel and join results | WASI is single-threaded. There is no runtime to schedule concurrent futures. Attempting `join!` or `select!` across `call-service` host function calls from inside a WASM component produces either a deadlock or a build error. The WASM Component Model async (WASI 0.3 / Preview 3) is not yet stable enough to depend on for production. | Chain sequential service calls. For true parallelism, the orchestrating service emits separate triggers that fire services independently, then aggregates results via KV in a final step. This is the pattern Cloudflare Workflows uses for parallel branches. |
| Bidirectional / streaming service calls | Service A calls service B, which streams intermediate results back | WAVS components are batch (trigger → result). Streaming into a component is not modeled in the current WIT interface and would require deep engine changes. WASI 0.3 async streams may eventually support this. | Structure as request/response: service A calls service B, waits for completion, gets the full result. For long-running operations, service B returns a job ID and service A polls via a continuation step. |
| Arbitrary call depth / unbounded recursion | Developers want to build tree-structured agent graphs without depth limits | Unbounded recursion means unbounded fuel and memory consumption. A single adversarial or buggy agent can crash the operator node. Wasmtime's fuel mechanism is per-invocation, not per-call-graph. | Enforce a configurable max call depth in `AllowedServiceCalls` (e.g. `max_depth: 3`). The engine tracks depth on the call stack and hard-stops at the limit. Fail loudly with a clear error message. |
| Spawning new trigger chains from inside a component | Service A fires a new EVM trigger from inside its execution, starting a new async workflow in the background | This requires the component to have write access to the trigger subsystem — a significant privilege beyond what components should have. It bypasses the signed-result model (the spawned trigger has no causal link to the signing event). | Return multiple `WasmResponse` entries from `run()` (already supported). Each response can encode a subsequent action for the aggregator to pick up. For true async fanout, use the existing cron or webhook trigger mechanisms at the service level. |
| Global state shared between services | Service A writes to a shared KV namespace that service B reads, as a side-channel for coordination | KV is per-component by default in WAVS (keyed by service ID). Shared namespaces create implicit coupling, make auditing impossible, and open privilege escalation vectors (service B can observe or clobber service A's state). | Explicit `call-service` with structured return types. If two services need to share state, one should own it and the other should read it via the `call-service` RPC. This keeps the data flow explicit and auditable. |
| Native HTTP calls from callee to caller (callbacks) | Service B wants to call back to service A's HTTP endpoint to signal completion | This requires service B to know service A's address, which breaks the composability model and introduces network-level coupling. In a multi-operator network, the "address" is not well-defined. | Callee returns a result to the caller synchronously via `call-service`. If the caller needs to react to the result, it does so in the next continuation step. Push vs. pull: always pull (caller drives), never push (callee initiates). |

---

## Feature Dependencies

```
Agent Continuation Mode (Continue/Done WIT variants)
    └──requires──> WIT interface change: new AgentStep return type in operator.wit
    └──requires──> Engine re-invocation loop (detect Continue, re-invoke same component)
    └──requires──> KV auto-persistence of continuation state (uses existing wasi:keyvalue)
    └──requires──> Max-step enforcement (new field in service.json + engine counter)
    └──enables──> Developer-defined multi-step workflows (convention inside component)
    └──enables──> LLM-decided autonomous continuation (agent returns Continue with next action)

Auto-persist continuation state
    └──requires──> Existing wasi:keyvalue host function (already in WAVS)
    └──requires──> Existing WavsMemory (already in wavs-rig)
    └──requires──> Agent Continuation Mode (above)
    └──enables──> Crash-resumable multi-step agents

Service-to-service synchronous RPC (call-service host function)
    └──requires──> New host function in operator WIT world (call-service)
    └──requires──> Engine inter-service dispatch (look up target service, execute its component)
    └──requires──> AllowedServiceCalls permission check (caller's service.json allowlist)
    └──requires──> Existing execute_operator_component (reused for callee execution)
    └──enables──> Supervisor/specialist agent patterns
    └──enables──> Service graph composition

AllowedServiceCalls permission (service.json)
    └──requires──> Service-to-service RPC (above)
    └──requires──> Existing AllowedHostPermission pattern (mirrors it)
    └──enables──> Default-deny service call security model

Composable trust-tier service calls (call-service with trust tier param)
    └──requires──> Service-to-service RPC (above)
    └──requires──> Existing three trust tiers in wavs-mcp + engine
    └──complexity──> HIGH — deferred to v3.x
```

### Dependency Notes

- **WIT interface change is the first hard blocker for continuation.** `operator.wit` must be extended with the `Continue`/`Done` return variants before any engine or SDK work can proceed. This is a versioned interface change (new WIT package version) and affects all downstream bindings.
- **Engine re-invocation loop is sequential with the WIT change.** Must wait for new WIT to generate correct bindings, then implement the loop in `execute_operator_component`.
- **`call-service` host function requires a new host function registration in the Wasmtime linker.** The engine must look up the target service's component, build `InstanceDeps` for it, call `execute_operator_component`, and return the result to the caller — all within the caller's execution timeout. This is the highest-complexity item.
- **AllowedServiceCalls is logically independent of continuation** but should ship together with `call-service` — shipping RPC without permission enforcement is a security regression.
- **Auto-persist state can be prototyped before continuation** since it only requires wasi:keyvalue, which already exists. But the persistence key schema needs to be decided once the WIT interface shape is known.
- **Max-step limits must ship with continuation.** Shipping continuation without step limits is unsafe for production operators.

---

## MVP Definition

### Launch With (v3.0)

Minimum to validate multi-step agents and service composition.

- [ ] `Continue`/`Done` WIT return variants in `operator.wit` (new WIT package version) — the foundation for everything; no continuation without this
- [ ] Engine re-invocation loop for `Continue` responses — detect variant, persist state, re-invoke same component with continuation payload as new trigger data
- [ ] Auto-persist continuation state to KV between steps (using existing `wasi:keyvalue`) — developers must not have to write checkpoint code manually
- [ ] Max-step enforcement (`max_continuation_steps` in service.json, engine counter, hard error at limit) — table stakes safety guard; without this a buggy agent can loop forever
- [ ] `call-service` host function in operator WIT world — synchronous RPC to any deployed WAVS service; the composability primitive
- [ ] `AllowedServiceCalls` in service.json + engine enforcement — default-deny; calling an unlisted service returns a typed error, not a crash
- [ ] Engine inter-service dispatch reusing `execute_operator_component` — avoids reimplementing execution machinery for callee services

### Add After Validation (v3.x)

Features to add once multi-step and RPC are stable and in use.

- [ ] `call-service` with trust tier parameter (result only / signed / on-chain) — enables verifiable composition; complex but high value for DeFi use cases
- [ ] `read_continuation_state()` helper in `wavs-rig` — ergonomic shorthand for the common pattern of reading persisted state at step start
- [ ] Activity feed UI: multi-step trace (show step N of M, state at each step, intermediate results) — observability for continuation chains; requires Tauri frontend work
- [ ] `call-service` call depth enforcement (`max_depth` in AllowedServiceCalls) — prevents unbounded recursion in service graphs

### Future Consideration (v4+)

Defer until the composition model is validated in production.

- [ ] Async parallel service calls (requires WASI 0.3 async / Preview 3 stabilization — not stable as of 2026)
- [ ] Shared KV namespaces between services (high complexity, auditing concerns, requires explicit governance model)
- [ ] Service graph visualizer in the Tauri app (meaningful only once users have built multi-service graphs worth visualizing)

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority | Requires |
|---------|------------|---------------------|----------|---------|
| `Continue`/`Done` WIT variants | HIGH (blocker) | MEDIUM | P1 | WIT versioning |
| Engine re-invocation loop | HIGH (blocker) | HIGH | P1 | WIT variants above |
| Auto-persist continuation state | HIGH | MEDIUM | P1 | wasi:keyvalue (existing) |
| Max-step enforcement | HIGH (safety) | LOW | P1 | Engine loop above |
| `call-service` host function | HIGH | HIGH | P1 | WIT world extension |
| `AllowedServiceCalls` permission | HIGH (safety) | MEDIUM | P1 | call-service above |
| Engine inter-service dispatch | HIGH (blocker) | HIGH | P1 | call-service above |
| Developer-defined step sequencing | MEDIUM | LOW | P1 | Continuation (convention, not engine) |
| `read_continuation_state()` helper | MEDIUM | LOW | P2 | Continuation shipped |
| `call-service` trust tier param | HIGH | HIGH | P2 | call-service + trust tier infra |
| Activity feed multi-step UI | MEDIUM | HIGH | P2 | Continuation + Tauri |
| Call depth enforcement | MEDIUM | LOW | P2 | call-service shipped |
| Parallel service calls (WASI 0.3) | HIGH | HIGH | P3 | WASI Preview 3 stabilized |

---

## Competitor / Analogous System Analysis

| Feature | LangGraph | Temporal | Cloudflare Workflows | wasmCloud | WAVS v3.0 |
|---------|-----------|----------|---------------------|-----------|-----------|
| Multi-step continuation | State machine + graph edges | Workflow functions (durable replay) | Step-based with auto-persist | Actor messages | Continue/Done variants in WIT |
| State persistence | Thread-level checkpoints (every node) | Event sourced replay | Automatic per-step | Actor in-memory + KV | KV auto-persist per Continue |
| Step limits | `recursion_limit` config | Workflow timeouts + activity retries | Step count limit | None explicit | `max_continuation_steps` in service.json |
| Service-to-service call | Agent tool calls external API | Activity calls other workflows | Workers RPC stubs (wRPC) | wRPC over NATS | `call-service` host function (synchronous) |
| Permission model | None (process-level) | Activity permissions (role-based) | Worker bindings (explicit) | Capability provider allowlist | AllowedServiceCalls allowlist in service.json |
| Sandbox | None | JVM process | V8 isolate | Wasmtime | Wasmtime (per-component) |
| Cryptographic trust | None | None | None | None | Operator signing (inherits from WAVS) |
| On-chain integration | None | None | None | None | EVM/Cosmos via host functions |

**Key takeaway:** The continuation + RPC pattern is well-established in Temporal, Cloudflare, and LangGraph. WAVS's differentiator is applying this pattern inside a cryptographically-verified, sandboxed WASM runtime with on-chain integration. The implementation patterns (KV checkpoints, step limits, explicit allowlists) are drawn from these established systems and are therefore low-risk choices.

---

## Sources

- [Cloudflare Workflows — durable execution GA](https://blog.cloudflare.com/workflows-ga-production-ready-durable-execution/) — MEDIUM confidence (official Cloudflare docs; confirms auto-persist, step-based model, agent trigger patterns)
- [Cloudflare Workflows — rearchitect for agentic era](https://blog.cloudflare.com/workflows-v2/) — MEDIUM confidence (official; confirms shift from human-triggered to agent-triggered workflows)
- [wasmCloud RPC docs](https://wasmcloud.com/docs/hosts/lattice-protocols/rpc/) — MEDIUM confidence (official wasmCloud docs; confirms wRPC, actor-to-actor call patterns)
- [LangGraph ReAct agent — recursion limit and max_iterations](https://python.langchain.com/v0.1/docs/modules/agents/how_to/max_iterations/) — MEDIUM confidence (official LangChain docs; confirms step limit is table stakes)
- [AI Agent Workflow Checkpointing — Zylos Research](https://zylos.ai/research/2026-03-04-ai-agent-workflow-checkpointing-resumability) — LOW confidence (single source; consistent with Cloudflare and Temporal patterns)
- [NVIDIA practical sandboxing guidance for agentic workflows](https://developer.nvidia.com/blog/practical-security-guidance-for-sandboxing-agentic-workflows-and-managing-execution-risk/) — MEDIUM confidence (official NVIDIA blog; confirms default-deny + allowlist as baseline security)
- [WASM Component Model async timeline](https://github.com/WebAssembly/component-model/issues/316) — HIGH confidence (upstream GitHub issue; confirms async in WASM is not stable as of 2026; justifies deferring parallel service calls)
- [WAVS PROJECT.md](../PROJECT.md) — HIGH confidence (project spec; primary source for v3.0 scope and existing infrastructure)
- [WAVS operator.wit](../../wit-definitions/operator/wit/operator.wit) — HIGH confidence (read directly; current WIT interface; baseline for the continuation variant extension)
- [WAVS engine execute_operator_component](../../packages/wavs/src/subsystems/engine/wasm_engine.rs) — HIGH confidence (read directly; existing execution path that call-service dispatch will reuse)
- [rig-wasi PromptHook / HookAction](../../packages/rig-wasi/src/agent/prompt_request/hooks.rs) — HIGH confidence (read directly; existing Continue/Terminate hook pattern in rig-wasi informs the WIT Continue/Done naming)

---

*Feature research for: WAVS v3.0 — agent continuation mode + service-to-service RPC*
*Researched: 2026-04-20*
