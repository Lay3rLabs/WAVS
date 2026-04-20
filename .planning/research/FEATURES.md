# Feature Research

**Domain:** WASM Agent SDK — rig-core integration into WAVS WASI sandbox (v2.0)
**Researched:** 2026-04-20
**Confidence:** MEDIUM (rig internals from official docs + crate docs; WASM blocker details from upstream GitHub issues and PROJECT.md; ecosystem patterns from community research)

## Feature Landscape

### Table Stakes (Users Expect These)

Features developers expect from any LLM agent SDK integration. Missing these = the SDK is unusable or the developer writes all the boilerplate themselves anyway.

| Feature | Why Expected | Complexity | WAVS Dependency | Notes |
|---------|--------------|------------|-----------------|-------|
| WASI-compatible rig fork | rig-core unconditionally depends on reqwest (no wasm32-wasip2 support; reqwest issue #2979 open upstream) and tokio rt-multi-thread feature; neither compiles to wasm32-wasip2 as-is | HIGH | None — this is the hard pre-condition for everything else | ~300-500 line fork: make reqwest optional, make tokio rt-multi-thread optional, fix cfg inconsistencies. Ring/aws-lc use assembly that doesn't link with wasip2 — TLS must be disabled inside the component (TLS is handled by WAVS host at the network boundary). Without this, nothing compiles. |
| HTTP transport bridge for LLM API calls | Agents call LLM APIs over HTTP; WASI uses `wasi:http/outgoing-handler`, not reqwest; developers expect this to just work | HIGH | Existing `wasi:http` host function | Replace rig's reqwest transport with a thin wrapper over WAVS's `wasi:http` host function. This is the most critical bridge — without it no LLM call is possible from inside the sandbox. |
| Async execution shim | rig's agent loop is async; WASI components use `wstd::runtime::block_on` (single-threaded); developers expect async to work | MEDIUM | Existing `wstd` async model in WAVS components | rig assumes tokio multi-thread runtime; WASI is single-threaded. Must configure rig concurrency to 1 and ensure agent loop works within `block_on`. Existing WAVS components (e.g. kv-store, permissions) already demonstrate the `block_on` pattern. |
| Provider-agnostic LLM client | Developers expect to swap providers (OpenAI, Anthropic, Groq, Ollama) via config without code changes; rig provides this via `CompletionModel` trait | LOW | WAVS settings/API key storage | rig-core already abstracts 20+ providers behind a unified trait. The fork preserves this. Developers pick provider via configuration. |
| Tool trait implementation in WASM | Developers expect to define tools as typed Rust structs with `call()` method — rig's `Tool` trait (NAME, Args, Output, async `call()`) and `#[rig_tool]` proc-macro are the established ergonomic | LOW | None | Should work inside WASM once the transport issues are resolved. Tools don't make HTTP calls themselves; they call WAVS host functions or manipulate in-memory state. |
| Structured LLM output | Developers expect typed responses from LLM calls, not raw string parsing | LOW | None | rig's `extractor` module handles this via JSON Schema + typed deserialization. Inherits from rig-core once fork compiles. |
| System prompt + preamble | Every agent needs a system prompt defining its role and behavior | LOW | None | rig `AgentBuilder.preamble()` is the standard. Table stakes for any agent SDK. |
| Compile to wasm32-wasip2 and deploy | The deliverable is a `.wasm` component that runs on the WAVS node, not a native binary | HIGH | Existing `just wasi-build-*` pipeline and WAVS service deployment | All transitive dependencies must compile to wasm32-wasip2. Existing WAVS components demonstrate the full build → deploy → execute path; wavs-rig follows the same pipeline. |

### Differentiators (Competitive Advantage)

Features that make wavs-rig meaningfully different from running rig natively or on any other infrastructure.

| Feature | Value Proposition | Complexity | WAVS Dependency | Notes |
|---------|-------------------|------------|-----------------|-------|
| WAVS host functions as typed rig tools | Developers get KV get/set, EVM query, HTTP fetch, and structured logging as first-class rig `Tool` impls — no bridge code to write | MEDIUM | Existing `wasi:keyvalue`, `wasi:http`, `host::log` host functions | Each host function becomes a typed `Tool` struct (e.g. `KvGetTool`, `KvSetTool`, `EvmQueryTool`, `HttpFetchTool`, `LogTool`). Add with `.tool(KvGetTool)` on the agent builder. This is the "batteries included" story — agents call on-chain data without extra plumbing. |
| KV-backed conversation memory with token budget | Persistent conversation history across triggers, auto-truncated to token limit — without this every trigger starts cold and the agent has no context | MEDIUM | Existing `wasi:keyvalue` host function | Implement `ConversationStore` backed by KV: append messages, retrieve recent N messages, enforce token budget cap. Cross-trigger state persistence is what separates stateful agents from stateless components. Pattern mirrors OpenAI Agents SDK session memory, implemented via WAVS's existing KV primitive. |
| `AllowedHostPermission` LLM API network policy | Per-component network policy enforced at Wasmtime level — agents can only call LLM APIs explicitly listed in `service.json`, enforced at the host, not the component | LOW | Existing `AllowedHostPermission` (`All`/`Only`/`None`) in the WAVS engine | A rig agent deployed with `Only(["api.openai.com", "api.anthropic.com"])` cannot exfiltrate data to arbitrary hosts. This is structural — no other Rust agent framework enforces this at the sandbox level. Key differentiator vs. running rig natively. Already implemented in WAVS; wavs-rig just needs to document and demonstrate it. |
| Cryptographic result signatures | Agent outputs are signed by operators; any party can verify the result without re-running the computation or trusting anyone | LOW | Existing operator signing in aggregator | Inherits from WAVS — no new code in wavs-rig. The agent returns results normally; the aggregator signs them. Structural advantage over any agent running on a single untrusted process. |
| Multi-operator agent execution | Run the same agent across N independent operators with configurable quorum — no single point of failure or trust | LOW | Existing multi-operator execution + aggregator | Inherits from WAVS. Particularly valuable for high-stakes agent decisions (DeFi rebalancing, on-chain actions) where a single operator could be compromised. |
| Event-driven agent triggering | Agents fire on EVM logs, Cosmos events, cron schedules, or HTTP webhooks — not just request/response | LOW | Existing trigger subsystem | Inherits from WAVS. Native event-driven agents vs. polling loops in user code. Genuine differentiator vs. langchain-rust, llm-chain, or running rig natively — all of which are purely request/response. |
| ~30-line agent component | Developer should be able to build a working end-to-end agent in ~30 lines of Rust | MEDIUM | All wavs-rig pieces working cleanly | This is the "demo magnet" feature. An example component showing trigger → LLM reasoning → tool use → result in minimal code is what developers will share and evaluate the SDK by. Requires all other pieces to land cleanly first. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Concurrent tool execution | Looks like a performance win; rig has concurrency configuration | WASI is single-threaded; concurrent async tasks do not exist in the same way. Attempting this produces confusing panics or silent deadlocks. The performance gain is marginal — agent tool calls are latency-bound on the LLM response, not parallel execution. | Set rig concurrency to 1. Sequential tool execution is correct for WASI MVP. Document this explicitly so developers don't attempt to tune it. |
| Streaming LLM responses | Developers familiar with OpenAI streaming want it for responsiveness | `wasi:http` response streaming is complex to implement correctly; the WAVS component execution model is batch (trigger → run → result). Partial streaming mid-execution does not map to the result submission model. | Buffer the full completion. For observability, structured traces (WAVS_IMPROVEMENTS.md #10, future milestone) give similar insight into agent progress without streaming complexity. |
| RAG / vector store integration | rig has 10+ vector store integrations (MongoDB, LanceDB, Qdrant, etc.); developers will ask for them | Vector stores require persistent TCP connections, complex auth, and large dependency trees that almost certainly do not compile to wasm32-wasip2. Wrong level of abstraction for the sandbox. | Use KV-backed conversation memory with a token budget. For embedding-based retrieval, call an external embedding API (via HTTP tool) and implement nearest-neighbor in application code. |
| Dynamic tool discovery at runtime | rig supports semantic tool retrieval from vector stores | Requires running embedding model and vector store inside the sandbox — infeasible for the same reasons as RAG. Also means the agent toolset is not statically auditable, which undermines the verifiability story. | Define tools statically in the component. The toolset is part of the component's interface and should be explicit and auditable. |
| Auto-spawning sub-agents inside a component | Multi-agent delegation inside a single component execution | The service-to-service call mechanism (WAVS_IMPROVEMENTS.md #7) is not yet built. Attempting to spawn sub-agents means re-implementing inter-component calls inside the component, bypassing sandbox boundaries and auditing. | Compose via WAVS service-to-service calls when that feature ships (subsequent milestone). For v2.0, single-agent components with a clear tool boundary. |
| langchain-rust or llm-chain instead of rig | Some developers may prefer other Rust LLM frameworks | langchain-rust has a heavier dependency footprint and is a Python LangChain port — less WASM-focused. llm-chain is less actively maintained. Neither has rig's 20+ provider coverage or WASM-compat traits. | Commit to rig. The WASM-compat traits (`WasmCompatSend`, `WasmCompatSync`, `HttpClientExt`) in rig-core exist precisely for this use case and are maintained upstream. The fork is ~300-500 lines of isolated platform patches, not a rewrite. |
| Full tokio runtime inside WASM | Some developers want to spawn tokio tasks, use `tokio::select!`, etc. | tokio's `rt-multi-thread` feature does not compile to wasm32-wasip2. The separate `tokio_wasi` crate is limited and not mainline. Attempting full tokio inside WASI leads to linker errors or runtime panics. | Use `wstd::runtime::block_on` as the single async executor. Structure the agent loop to be sequential. Document this constraint with a clear error message so developers don't hit it silently. |

---

## Feature Dependencies

```
WASI-compatible rig fork  [HARD BLOCKER — must land first]
    └──enables──> HTTP transport bridge
    └──enables──> Async execution shim (wstd block_on compatibility)
    └──enables──> Tool trait implementation in WASM
    └──enables──> Structured LLM output (rig extractor)
    └──enables──> Compile to wasm32-wasip2

HTTP transport bridge
    └──requires──> WASI-compatible rig fork
    └──requires──> Existing wasi:http host function (already in WAVS)
    └──enables──> All LLM API calls from inside sandbox
    └──enables──> Provider-agnostic client (OpenAI, Anthropic, Groq, etc.)

WAVS host functions as typed rig tools
    └──requires──> Tool trait impl in WASM (via rig fork)
    └──requires──> Existing wasi:keyvalue, wasi:http, host::log (already in WAVS)
    └──enables──> KvGetTool, KvSetTool, EvmQueryTool, HttpFetchTool, LogTool

KV-backed conversation memory
    └──requires──> Existing wasi:keyvalue host function (already in WAVS)
    └──enhances──> Multi-turn agent reasoning across triggers

~30-line example component
    └──requires──> All of the above working end-to-end

AllowedHostPermission LLM network policy
    └──requires──> Existing engine AllowedHostPermission (already in WAVS)
    └──enhances──> HTTP transport bridge (restricts which LLM APIs can be called)
    └──is documented and demonstrated in example service.json

Cryptographic result signatures
    └──requires──> Existing operator signing (already in WAVS)
    └──requires──> No new code in wavs-rig

Multi-operator execution
    └──requires──> Existing aggregator (already in WAVS)
    └──requires──> No new code in wavs-rig

Event-driven triggering
    └──requires──> Existing trigger subsystem (already in WAVS)
    └──requires──> No new code in wavs-rig
```

### Dependency Notes

- **WASI-compatible rig fork is the single hard blocker.** Everything else depends on it. It is the first thing to build and must be validated before any other work starts.
- **HTTP transport bridge and async shim are sequential with the fork.** Once rig compiles to wasm32-wasip2, replace the reqwest transport with a wasi:http wrapper. These two steps cannot be parallelized.
- **KV-backed memory is logically independent.** Can be designed before the fork lands; only requires wasi:keyvalue which already exists in WAVS. Can be developed in parallel.
- **WAVS host function tools are independent of the fork.** The tool structs call WAVS host functions, not LLM APIs. Can be designed and stubbed in parallel; integration-tested once the fork lands.
- **Security and trust features (network policy, signing, multi-operator) require zero new code in wavs-rig.** They inherit from WAVS. The work is documentation and example configuration.

---

## MVP Definition

### Launch With (v2.0)

Minimum to validate the agent runtime concept and give developers something real to build with.

- [ ] WASI-compatible rig fork — hard dependency; nothing else works without it
- [ ] `wavs-rig` integration crate with HTTP transport bridge and async shim — minimum to make LLM calls from inside the WASM sandbox
- [ ] WAVS host functions as typed rig tools (KV get/set, HTTP fetch, structured logging) — batteries included; without this developers must write all the bridge code themselves
- [ ] KV-backed conversation memory with token budget — stateless agents that forget everything on each trigger are not useful agents
- [ ] Example agent component (trigger → LLM reasoning → tool use → result, ~30 lines) — validates the full stack and serves as the "can I build something in an afternoon?" reference

### Add After Validation (v2.x)

- [ ] EVM query tool — after basic agents work, on-chain data access is the next most-requested capability for WAVS's target audience
- [ ] Structured output via rig's extractor module — once basic completions work; typed responses improve reliability for production agents
- [ ] Agent observability in the Tauri app (reasoning chain display) — significant UX improvement once developers have agents to observe; see WAVS_IMPROVEMENTS.md #10

### Future Consideration (v3+)

- [ ] Service-to-service calls (inter-component RPC) — WAVS_IMPROVEMENTS.md #7 is the prerequisite; enables supervisor/specialist agent patterns
- [ ] `Continue` execution mode for multi-step agents — WAVS_IMPROVEMENTS.md #5; requires WIT interface changes and engine changes; significant runtime work beyond this milestone
- [ ] App-level agent workflow builder (template gallery, intent-driven config) — WAVS_IMPROVEMENTS.md #9; high UX value but requires the agent runtime to be stable first
- [ ] Cost tracking (LLM API token usage per execution) — valuable for production operators; requires structured trace format first

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority | WAVS Dependency |
|---------|------------|---------------------|----------|-----------------|
| WASI-compatible rig fork | HIGH (blocker) | HIGH | P1 | None |
| HTTP transport bridge | HIGH (blocker) | HIGH | P1 | Existing wasi:http |
| Async execution shim | HIGH (blocker) | MEDIUM | P1 | Existing wstd |
| WAVS host functions as typed tools | HIGH | MEDIUM | P1 | Existing host functions |
| KV-backed conversation memory | HIGH | MEDIUM | P1 | Existing wasi:keyvalue |
| ~30-line example component | HIGH | LOW (once above land) | P1 | All of above |
| AllowedHostPermission LLM policy (doc + demo) | MEDIUM | LOW (already exists) | P1 | Already in engine |
| Cryptographic signatures (doc) | MEDIUM | LOW (already exists) | P1 | Already in aggregator |
| EVM query tool | MEDIUM | LOW | P2 | Existing EVM host access |
| Structured output extractor | MEDIUM | LOW | P2 | rig-core extractor module |
| Agent observability in app | MEDIUM | HIGH | P2 | Agent SDK + Tauri |
| Service-to-service calls | HIGH | HIGH | P3 | Not yet built |
| Multi-step Continue execution mode | HIGH | HIGH | P3 | Not yet built |

**Priority key:**
- P1: Must have for v2.0 launch
- P2: Add in v2.x after core validated
- P3: Future milestone — requires prerequisite features not in this milestone

---

## Competitor Feature Analysis

| Feature | rig-core (native) | langchain-rust | wavs-rig approach |
|---------|-------------------|----------------|-------------------|
| LLM provider coverage | 20+ via unified `CompletionModel` trait | Limited, Python LangChain port | Inherits rig's 20+ providers |
| WASM/WASI support | Partial (wasm32-unknown-unknown for browser); wasm32-wasip2 blocked by reqwest | None documented | Fork for wasm32-wasip2 specifically; the target no other framework addresses |
| Tool calling | `Tool` trait + `#[rig_tool]` proc-macro; fluent `.tool()` builder | LangChain-style tools | Inherits rig's pattern + adds WAVS host function tools pre-built |
| Memory/state | Conversation module (in-process) | Various | KV-backed persistent memory across trigger invocations |
| Security sandbox | None (process-level only) | None | Wasmtime sandbox + `AllowedHostPermission` network policy |
| Cryptographic trust | None | None | Operator signatures + configurable multi-operator quorum |
| Event-driven triggers | None (request/response only) | None | EVM, Cosmos, cron, HTTP triggers via WAVS |
| On-chain interaction | None | None | EVM/Cosmos host functions as typed rig tools |
| Dev experience | `AgentBuilder` fluent API; ~30-50 lines for basic agent | More verbose; less ergonomic | `wavs-rig` wraps rig's AgentBuilder with WAVS context; same ~30-line target |

---

## Sources

- [Rig documentation — Agents concept](https://docs.rig.rs/docs/concepts/agent) — MEDIUM confidence (official docs)
- [Rig documentation — Tools concept](https://docs.rig.rs/docs/concepts/tools) — MEDIUM confidence (official docs)
- [Rig quickstart — Tools](https://docs.rig.rs/docs/quickstart/tools) — MEDIUM confidence (official docs)
- [rig-core on docs.rs](https://docs.rs/rig-core/latest/rig/) — MEDIUM confidence (crate docs; wasm_compat module and if_wasm/if_not_wasm macros confirmed)
- [reqwest issue #2979 — wasm32-wasip2 support open](https://github.com/seanmonstar/reqwest/issues/2979) — HIGH confidence (upstream issue, confirms blocker)
- [reqwest issue #891 — blocking not available with wasm32](https://github.com/seanmonstar/reqwest/issues/891) — HIGH confidence (upstream confirmed, long-standing)
- [tokio issue #4827 — stabilize WASI support](https://github.com/tokio-rs/tokio/issues/4827) — HIGH confidence (upstream tracker)
- [WAVS PROJECT.md](../PROJECT.md) — HIGH confidence (project spec; primary source for WAVS capabilities, v2.0 scope, and fork size estimate)
- [WAVS_IMPROVEMENTS.md](../../WAVS_IMPROVEMENTS.md) — HIGH confidence (detailed agent feature spec with sequencing)

---

*Feature research for: wavs-rig agent SDK (WAVS v2.0 milestone)*
*Researched: 2026-04-20*
