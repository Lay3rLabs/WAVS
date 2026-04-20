# Pitfalls Research

**Domain:** rig-core integration into WAVS WASI sandbox (v2.0 Agent Runtime)
**Researched:** 2026-04-20
**Confidence:** HIGH — based on direct codebase inspection, rig-core source review, and verified ecosystem findings

## Critical Pitfalls

### Pitfall 1: Unconditional tokio::rt Feature Causes Linker Errors in WASI Target

**What goes wrong:**
rig-core's default feature set enables `tokio/rt` (the Tokio thread-based runtime). The `wasm32-wasip2` target has no OS thread model and Wasmtime provides no Tokio runtime. Linking fails with `__wasi_thread_spawn` or similar unresolved symbol errors at WASM component link time, not at `cargo build`. The error is not a Rust compile error — it surfaces only when `wasm-tools component new` or `cargo component build` assembles the binary, making it look like a toolchain issue rather than a dependency problem.

**Why it happens:**
Developers assume that because `cargo build --target wasm32-wasip2` succeeds, the component is usable. The actual WASI component assembly step runs the linker separately, which is when thread-related symbols are resolved. rig-core pulls in `tokio` with the `rt` feature unconditionally because its async trait bounds require a Tokio executor. The fork must patch this before any integration work can be tested end-to-end.

**How to avoid:**
In the rig-core fork, gate `tokio/rt` behind a non-default feature like `tokio-rt`. Add `#[cfg(not(target_arch = "wasm32"))]` guards on any `tokio::spawn` or `Runtime::new()` calls. The WAVS sandbox uses `wstd::runtime::block_on` as the only executor — no Tokio runtime is created inside the component. Confirm the fork compiles and produces a valid component with `cargo component build --target wasm32-wasip2` before touching any rig API surface.

**Warning signs:**
- `cargo build --target wasm32-wasip2` succeeds but `cargo component build` fails with linker errors
- Any mention of `__wasi_thread_spawn`, `pthread`, or `mmap` in link errors from the WASI component
- `wasm-tools validate` passes the raw `.wasm` but `wasm-tools component new` fails

**Phase to address:**
Phase 1 (rig-core fork) — this is the first blocker. Nothing downstream can be verified until this is resolved.

---

### Pitfall 2: Nested `block_on` Panics — rig Agent Loop Inside Existing Executor

**What goes wrong:**
Existing WAVS components call `wstd::runtime::block_on(async { ... })` as their top-level async entry point. If the rig agent loop internally calls `block_on` a second time (e.g., to drive an inner async request), the program panics with "cannot call `block_on` from within an async context" or produces undefined behavior in the wstd single-threaded executor. This is WASM-specific: unlike Tokio where `block_in_place` or `spawn_blocking` provide escape hatches, wstd has no such mechanism. The nested call starves the outer executor.

**Why it happens:**
rig's `Agent::prompt()` method is `async fn` throughout. When a developer wraps the rig agent call in a closure inside `block_on`, they are calling async code from inside an async context — correct. But if any rig-internal code (e.g., retry logic, tool dispatch, provider client internals) calls `block_on` recursively, the single-threaded WASI executor deadlocks or panics. The rig Cloudflare Worker compatibility feature (PR #175) added synchronous wrappers for this exact reason in a different constrained environment.

**How to avoid:**
The entire agent execution — from trigger receipt through tool calls to final LLM response — must run inside a single `block_on` at the component entry point. The rig fork must ensure that no internal code path calls a blocking executor. Search the forked rig-core for any use of `tokio::runtime::Handle::current().block_on(...)`, `futures::executor::block_on(...)`, or `wstd::runtime::block_on(...)` in non-entry-point positions. The integration crate's async shim must use `wstd` primitives only, never create inner executors.

**Warning signs:**
- Component panics with "already in async context" or deadlocks indefinitely on first LLM call
- Removing the outer `block_on` and making the entry point a plain `async fn` fixes the panic — this confirms nested `block_on` is the cause
- Wasmtime engine reports `Trap::Interrupt` but fuel is not exhausted — the component is stuck, not out of budget

**Phase to address:**
Phase 1 (rig-core fork) — validate with a minimal async probe before building the HTTP bridge.

---

### Pitfall 3: reqwest HTTP Transport Not Available in WASI; `wasi:http` Bridge Required

**What goes wrong:**
rig-core's HTTP client trait (`HttpClientExt`) defaults to a reqwest backend. reqwest requires platform socket APIs (`connect`, `send`, `recv`) that do not exist in WASI p2. Compiling with the reqwest feature enabled causes linker errors on WASI even after the tokio patch. Even the reqwest WASM browser target (`wasm32-unknown-unknown`) cannot be used — that target uses browser `fetch`, not `wasi:http/outgoing-handler`. The two WASM targets are not interchangeable.

**Why it happens:**
Developers see "reqwest has WASM support" and assume WASI p2 is covered. The reqwest WASM support is for `wasm32-unknown-unknown` (browser JS environment) only — it uses `wasm-bindgen` to call `window.fetch`. WASI p2 has no JavaScript bridge. The correct transport is the `wasi:http/outgoing-handler` host function, which Wasmtime provides via `wasmtime-wasi-http`. WAVS already links this host function (`configure_linker` in `packages/engine/src/worlds/instance.rs` enables HTTP based on `AllowedHostPermission`).

**How to avoid:**
In the rig-core fork, gate reqwest behind a `#[cfg(not(target_arch = "wasm32"))]` feature and add a `WasiHttpTransport` implementation behind `#[cfg(target_arch = "wasm32")]`. The `wavs-rig` integration crate must implement rig's HTTP transport trait using the WIT-generated `wasi::http::outgoing_handler::handle()` bindings, not any OS socket API. The existing `wasmtime_wasi_http::WasiHttpCtx` host implementation in WAVS can be used for verification — if the component can call `wasi:http` successfully, the bridge is wired correctly.

**Warning signs:**
- Linker error mentioning `__wasi_sock_connect` or TLS symbol from `rustls/ring` — reqwest native is still linked
- LLM API calls succeed in Wasmtime simulation but fail when deployed on a WAVS node with `AllowedHostPermission::None` — the HTTP permission policy is not being respected
- The test component uses `reqwest::Client::new()` rather than the WIT-generated outgoing-handler — this will compile locally but fail at WASI validation

**Phase to address:**
Phase 2 (wavs-rig integration crate, HTTP transport bridge) — after fork compiles, before any real LLM calls.

---

### Pitfall 4: Fork Divergence — Upstream rig Releases Break the Fork Silently

**What goes wrong:**
The rig-core fork starts as a ~300-500 line diff. Over time, upstream rig ships new LLM providers, breaking API changes (e.g., rig v0.31 changed `CompletionResponse` to add `message_id`, removed `AgentBuilderSimple`, changed `StreamingChat` traits), and dependency bumps. The fork does not receive these changes automatically. Developer agents using the fork miss new providers. Worse: if the fork's Cargo.toml specifies a version range that overlaps with the upstream crate name, `cargo update` may silently switch between the fork and upstream depending on lock file state.

**Why it happens:**
`[patch]` entries in Cargo.toml override the upstream crate only in the workspace that declares the patch. If a downstream demo project adds rig as a dependency without inheriting the workspace patch, it pulls unpatched upstream rig. Cargo does not warn about this. The lock file difference is the only signal, and only if the developer compares lock files.

**How to avoid:**
Publish the fork to a private or public git repository and pin to an explicit commit SHA in `Cargo.toml` (`git = "...", rev = "abc1234"`). Document the upstream rig version the fork is based on in a `FORK_BASIS.md` at the fork root. Create a tracking issue for upstream rig WASM support (the stated goal is to upstream these patches). Before each WAVS release, diff the fork against the corresponding upstream tag and selectively backport provider additions. Never use `version = "..."` for the fork dependency — always `git + rev`.

**Warning signs:**
- `cargo tree -p rig-core` shows two different rig versions in the dependency graph — upstream leaked in through a transitive dep
- A new LLM provider available in upstream rig cannot be used without manual backport
- CI passes locally (with workspace `[patch]`) but fails in a demo project that does not inherit the workspace

**Phase to address:**
Phase 1 (fork setup) — establish the pinning strategy before any integration code is written.

---

### Pitfall 5: KV State Serialization Format Lock-In — History Grows Unboundedly

**What goes wrong:**
Agent conversation history is serialized to JSON and stored in the WAVS KV store (one key per service instance, e.g., `"conversation:{service_id}"`). As the agent runs over many invocations, the history array grows without bound. At some invocation, the deserialized history exceeds the LLM's context window, causing an `invalid_request_error: context_length_exceeded` from the provider API. The agent cannot recover because it cannot write new state if history read + LLM call fails mid-invocation. The KV store key is now "poisoned" — every future invocation fails immediately.

**Why it happens:**
Developers implement history persistence as a simple append-only JSON array. There is no eviction policy, no token count check, and no trimming. The first 20-50 invocations work fine, then failure appears seemingly at random. Because WAVS components are stateless between invocations, there is no in-memory recovery path — the bad state lives in the KV store indefinitely.

**How to avoid:**
The KV memory module in `wavs-rig` must enforce a token budget at write time, not read time. Before appending a new exchange, measure the token count of the existing history plus the new messages. If the sum exceeds a configurable `max_history_tokens` threshold (default: 75% of the model's context window), trim the oldest messages from the front of the array until the budget is met. Store a `version` field in the serialized state so future format changes can migrate gracefully. Test with a history that is exactly at the token limit, exactly one token over, and exactly two exchanges over.

**Warning signs:**
- Agent works correctly for the first N invocations, then always returns an error containing "context_length_exceeded" or "max_tokens"
- The KV key for the service still has the old (failing) value after a deployment — clear the KV namespace to confirm the fix is in the write path, not read
- History serialized to KV exceeds 1 MB for a single service instance

**Phase to address:**
Phase 3 (KV-backed conversation memory) — token budget management is not optional for production use.

---

### Pitfall 6: Fuel Exhaustion vs. Epoch Interruption — Agent Loop Has No Budget for Long Reasoning

**What goes wrong:**
WAVS uses both fuel metering (instruction count budget) and epoch interruption (wall-clock timeout) in combination, as visible in `packages/engine/src/worlds/instance.rs` and `packages/engine/src/worlds/operator/execute.rs`. An LLM agent with multi-step tool use (e.g., 5 LLM calls + 5 tool executions) consumes far more WASI instructions than a simple echo component. If the default fuel limit is configured for simple services, the agent runs out of fuel after 1-2 LLM round trips and returns `EngineError::OutOfFuel`. This is not a timeout — refactoring to be faster does not help. The fuel budget must be explicitly raised for agent workflows.

**Why it happens:**
The fuel setting in WAVS operator configuration is a single value applied to all service types. There is no per-service-type fuel policy. Developers see `OutOfFuel` and assume the component is in an infinite loop; they add timeout instrumentation that reveals the component finished quickly — confusing because `OutOfFuel` is a fuel trap, not a time trap. The existing error types `EngineError::OutOfFuel` and `EngineError::OutOfTime` are correct, but the distinction is not obvious to developers new to the engine.

**How to avoid:**
Document that agent components require a higher fuel budget than simple query components. The `service.json` or operator config for an agent service should set fuel to at least 10x the default (exact multiple depends on the number of tool call rounds). Add a log line at the start of each agent invocation showing current fuel remaining. The `InstanceDeps.store.get_fuel()` method is available at the host side — expose remaining fuel in the activity feed for agent services so operators can tune the limit empirically.

**Warning signs:**
- Agent returns `OutOfFuel` on first real LLM call, not after many rounds — the fuel limit is too low for even a single HTTP request through `wasi:http`
- Increasing the epoch timeout does not change the error — this is fuel, not wall-clock time
- Simple echo components work fine but any HTTP-making component fails with `OutOfFuel`

**Phase to address:**
Phase 2 (wavs-rig integration crate) — validate fuel budget requirements before example agent is built.

---

### Pitfall 7: AllowedHostPermission Not Validated Before Agent Deployment — Silent Network Failures

**What goes wrong:**
WAVS enforces network policy via `AllowedHostPermission` (`All` / `Only` / `None`) at the Wasmtime linker level. A component deployed with `AllowedHostPermission::None` attempting to call `wasi:http/outgoing-handler` traps immediately. The error is not surfaced as an LLM API error — it is a WASM trap (`EngineError::ComponentError`) that looks identical to any other component crash. The agent error message contains no hint that the network policy blocked the call.

**Why it happens:**
Developers configure `AllowedHostPermission` correctly in the service definition but forget to update it when changing the LLM provider endpoint (e.g., from OpenAI to a private Ollama instance). An Ollama instance running at `http://localhost:11434` is not reachable from a sandboxed component even with `AllowedHostPermission::All` — the sandbox has no loopback to the host machine. The permission system enforces host allowlists, but the developer mental model assumes it works like a network firewall rule, not a complete isolation boundary.

**How to avoid:**
The `wavs-rig` integration crate should validate the HTTP permission configuration at startup (first agent invocation) and return a structured error if `AllowedHostPermission::None` is set. Add an example agent `service.json` that shows the correct `allowed_hosts` configuration for each supported LLM provider (OpenAI, Anthropic, Groq, OpenRouter). Document clearly that Ollama cannot be used from inside a WASI component unless the WAVS node proxies requests to the local Ollama endpoint and exposes it as an allowed external URL.

**Warning signs:**
- `EngineError::ComponentError` with no inner LLM error message — the HTTP call never reached the network
- The agent works in simulation (`wavs simulate`) but fails when deployed — simulation may bypass permission checks
- Changing `AllowedHostPermission` from `None` to `All` in the service definition fixes the error

**Phase to address:**
Phase 3 (example agent component) — validate end-to-end with the correct network policy before any documentation.

---

### Pitfall 8: cfg Flag Inconsistencies in rig-core Fork — Silent Dead Code on WASI Target

**What goes wrong:**
rig-core's existing WASM compatibility traits (`WasmCompatSend`, `WasmCompatSync`) gate on `#[cfg(target_arch = "wasm32")]`. The `wasm32-wasip2` target reports `target_arch = "wasm32"` AND `target_os = "wasi"`. Some rig internal conditions check only `target_arch`, others check `target_family = "wasm"`, and some check `not(target_os = "wasi")`. On the fork, patches applied for WASI p2 may be gated incorrectly: a patch guarded by `#[cfg(target_arch = "wasm32")]` also activates on browser WASM targets, potentially breaking those. A patch guarded by `#[cfg(target_os = "wasi")]` does not activate on the `wasm32-wasip1` target (which reports `target_os = "wasi"` differently across Rust versions). The upstream rig Cloudflare Worker feature flag does not distinguish between `wasm32-wasip2` and `wasm32-unknown-unknown`.

**Why it happens:**
The Rust target triple system is not intuitive for WASM: `wasm32-unknown-unknown` (browser), `wasm32-wasip1` (WASI preview 1), and `wasm32-wasip2` (WASI preview 2) all have `target_arch = "wasm32"` but different `target_os`. Most library authors only test browser WASM. The recommended cfg for WASI p2 is `#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]`, but the narrower check `#[cfg(all(target_arch = "wasm32", not(target_family = "wasm")))]` silently activates no code on any WASM target.

**How to avoid:**
Establish a single canonical cfg alias at the fork root: `#[cfg(wavs_wasi_target)]` using `build.rs` to set `cargo:rustc-cfg=wavs_wasi_target` when `CARGO_CFG_TARGET_ARCH == "wasm32" && CARGO_CFG_TARGET_OS == "wasi"`. Use this alias exclusively in the fork's platform-shim code. This avoids per-file inconsistencies and is testable. Run `cargo check --target wasm32-unknown-unknown` on the fork to verify browser WASM compatibility is not broken by the WASI patches.

**Warning signs:**
- Platform-specific code in the fork does not activate on `wasm32-wasip2` despite the `#[cfg(target_arch = "wasm32")]` guard — check target_os
- The fork compiles on `wasm32-wasip2` but produces a component that traps immediately, before any user code runs — a cfg-gated shim is activating on the wrong target
- `cargo check --target wasm32-unknown-unknown` after applying the fork patches shows unexpected compilation failures in rig's browser-WASM path

**Phase to address:**
Phase 1 (rig-core fork) — establish the cfg alias as the first commit in the fork before any other patches.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode the LLM API base URL in the integration crate | Faster initial test | Cannot switch providers or route through a proxy | Never — use a config var from the WAVS service config at runtime |
| Use `serde_json::Value` for conversation history serialization | Simple, no schema | Format changes corrupt existing KV history; no migration path | Only for initial prototype; add a `version` field before shipping |
| Set fuel limit to `u64::MAX` for agent services | Eliminates OutOfFuel errors during development | Infinite loop in agent reasoning exhausts the host machine | Never in production; set a high but finite limit and test the fuel trap |
| Sequential tool calls with `block_on` inside the agent loop | Works on single-threaded WASI | rig concurrency setting of 1 is already the correct approach — this is fine | Always acceptable for MVP; concurrent tool calls are not needed |
| `[patch]` workspace override without pinning to git rev | Faster iteration during development | Lock file drift; demo projects outside the workspace pull upstream rig | Only acceptable during active fork development; pin before any deployment |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| rig `Agent::prompt()` | Calling from inside a second `block_on` | Drive the entire agent loop from a single top-level `block_on` at the WASM component entry point |
| `wasi:http/outgoing-handler` | Using reqwest native or the browser `fetch` target | Implement rig's HTTP transport trait using WIT-generated `wasi::http` bindings |
| KV bucket naming | Using a hardcoded bucket name like `"history"` | Namespace by service ID: `format!("agent:{service_id}")` to isolate per-service history |
| `AllowedHostPermission` | Deploying agent with `None` permission assuming it only controls untrusted external calls | `None` blocks ALL outgoing HTTP including LLM API calls; use `All` or a specific `Only` allowlist |
| Token budget check | Checking token count after writing to KV | Check and trim BEFORE writing; a failed LLM call after a write leaves the KV in a state with no new messages to recover from |
| Fuel metering | Assuming fuel applies uniformly | Each `wasi:http` outgoing call consumes orders of magnitude more fuel than pure computation; calibrate with realistic LLM round trips |
| rig fork version pinning | Using `version = "0.x"` in Cargo.toml | Use `git = "...", rev = "abc1234"` to prevent accidental upstream upgrades |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Unbounded conversation history in KV | Every invocation gets slower; eventually hits context limit error | Token budget with trim-from-front eviction at write time | At ~20-50 invocations for GPT-4-class models (128k context window) |
| Deserializing full history on every invocation | Read latency grows linearly with history length | Implement sliding window: store only last N exchanges, not full history | At >100 historical messages in the KV store |
| JSON serialization of large binary tool results | Component result payload explodes in size (hex encoding doubles size) | Apply the existing 4 KB cap from the aggregator to tool result storage in KV | Any tool result > 2 KB stored as hex in KV |
| LLM retries without backoff inside WASI | Rapid-fire retries exhaust fuel budget before epoch timeout fires | Implement exponential backoff with a max retry count of 2-3; the epoch interruption is the outer guard | On any provider rate-limit response (HTTP 429) |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Storing raw API keys in the agent's KV conversation history | Key visible to anyone with KV read access to the service namespace | Never log or store provider API keys in KV; read them only from WAVS config vars at invocation time |
| Logging full LLM prompt/response at INFO level | Prompts may contain user PII or sensitive trigger data | Log at DEBUG; emit only token counts and provider name at INFO |
| Using `AllowedHostPermission::All` for all agents without review | Agent can call arbitrary external URLs including internal network endpoints | Use `AllowedHostPermission::Only` with an explicit provider allowlist for production deployments |
| Passing raw trigger data directly into LLM prompt without sanitization | Prompt injection via EVM event data | Treat all trigger data as untrusted; wrap in a structured prompt template that limits where trigger data appears |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| OutOfFuel error with no context on why | Developer cannot distinguish "fuel too low" from "infinite loop" | Surface `OutOfFuel` vs `OutOfTime` distinction in the activity feed; show fuel consumed vs budget |
| Network permission trap looks identical to component crash | Developer spends time debugging component logic, not configuration | Add a startup validation that explicitly checks network permission and emits a structured error before the first LLM call |
| No indication of which tool call failed in a multi-tool agent loop | Debug requires log inspection, cannot see from the activity feed | Log each tool call and result at WAVS host level before returning to the rig agent |

## "Looks Done But Isn't" Checklist

- [ ] **Fork compiles:** `cargo component build --target wasm32-wasip2` succeeds AND `wasm-tools validate` passes — not just `cargo build`
- [ ] **No nested executors:** Confirm the integration crate has exactly one `block_on` call (at the WIT `run` entry point) — search for all `block_on` calls in the compiled component's source
- [ ] **HTTP transport wired:** A minimal agent that makes one LLM API call succeeds when deployed on a WAVS node with `AllowedHostPermission::All` — not just in simulation
- [ ] **Fuel budget validated:** Run a 5-round tool-use agent and confirm it completes without `OutOfFuel` at the configured fuel limit
- [ ] **KV token budget enforced:** Run an agent for 100+ invocations and confirm history does not grow unboundedly; confirm the 101st invocation succeeds
- [ ] **Fork pinned to git rev:** `cargo tree -p rig-core` shows the fork rev, not the upstream crates.io version, in all demo projects

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Tokio rt causes linker errors | LOW | Add `default-features = false` to rig-core fork dependency; add `#[cfg]` guards on tokio spawn sites; rebuild |
| Nested block_on deadlock | MEDIUM | Audit all async entry points in the integration crate; ensure single top-level block_on; no recovery possible mid-execution — component must be redeployed |
| KV history poisoned by context overflow | LOW | Delete the KV bucket for the affected service instance (`wasi:keyvalue store::delete`); next invocation creates a fresh history |
| Fuel exhaustion for agent services | LOW | Increase fuel limit in operator config for the specific service workflow; no code change needed |
| Fork diverged from upstream | HIGH | Use `git diff` against the upstream tag to identify non-platform patches; manually backport; test on both WASI and native targets |
| Network permission trap | LOW | Update service definition `allowed_hosts`; redeploy service; no code change to component |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Tokio rt linker errors | Phase 1 (rig-core fork) | `cargo component build --target wasm32-wasip2` with no linker errors |
| Nested block_on deadlock | Phase 1 (rig-core fork) | Minimal async probe test: one await inside block_on, no inner executors |
| reqwest not available in WASI | Phase 2 (HTTP transport bridge) | End-to-end LLM call via wasi:http outgoing-handler succeeds on deployed WAVS node |
| Fork divergence | Phase 1 (fork setup) | Git rev pinned in Cargo.toml; diff tracked in FORK_BASIS.md |
| KV history unbounded growth | Phase 3 (KV memory module) | 100-invocation soak test; history size stable after trim |
| Fuel exhaustion for agents | Phase 2 (integration crate) | 5-tool-call agent completes within fuel budget; OutOfFuel only on deliberate infinite loop |
| AllowedHostPermission silent trap | Phase 3 (example agent) | Startup validation emits structured error for None permission before first LLM call |
| cfg flag inconsistencies | Phase 1 (rig-core fork) | Single canonical `wavs_wasi_target` cfg alias; `cargo check` on both wasm32-wasip2 and wasm32-unknown-unknown |

## Sources

- Direct code inspection: `/workspace/WAVS/packages/engine/src/worlds/operator/execute.rs` — dual fuel + epoch timeout pattern, `EngineError::OutOfFuel` vs `OutOfTime`
- Direct code inspection: `/workspace/WAVS/packages/engine/src/worlds/instance.rs` — `EPOCH_YIELD_PERIOD_MS`, `AllowedHostPermission` linker configuration, `KeyValueCtx`
- Direct code inspection: `/workspace/WAVS/packages/engine/src/backend/wasi_keyvalue/context.rs` — per-namespace KV isolation, `KeyValueCtxProvider` trait
- Direct code inspection: `/workspace/WAVS/examples/components/echo-data/src/lib.rs` — `wstd::runtime::block_on` as sole async entry point pattern
- Direct code inspection: `/workspace/WAVS/examples/components/kv-store/src/lib.rs` — bucket open/read/write pattern used by components
- Direct code inspection: `/workspace/WAVS/.planning/PROJECT.md` — confirmed hard blockers: unconditional reqwest, tokio rt feature, cfg inconsistencies; fork size ~300-500 lines
- rig-core issue #176 (CF worker support): synchronized async wrappers via feature flag for constrained WASM environments — [github.com/0xPlaygrounds/rig/issues/176](https://github.com/0xPlaygrounds/rig/issues/176)
- rig v0.31 release notes: reqwest upgraded to 0.13 with rustls default; breaking API changes confirm active churn — [github.com/0xPlaygrounds/rig/discussions/1406](https://github.com/0xPlaygrounds/rig/discussions/1406)
- reqwest wasm32-wasip2 support issue (open, unresolved): [github.com/seanmonstar/reqwest/issues/2979](https://github.com/seanmonstar/reqwest/issues/2979)
- Wasmtime interrupting wasm (epoch vs fuel): [docs.wasmtime.dev/examples-interrupting-wasm.html](https://docs.wasmtime.dev/examples-interrupting-wasm.html)
- wstd async model and temporary nature: [github.com/bytecodealliance/wstd](https://github.com/bytecodealliance/wstd)
- Cargo patch publishing limitation (fork cannot be published to crates.io): [doc.rust-lang.org/cargo/reference/overriding-dependencies.html](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html)
- Rust target triple cfg for WASM: `wasm32-wasip2` reports `target_arch = "wasm32"` AND `target_os = "wasi"` — [doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html)

---
*Pitfalls research for: rig-core integration into WAVS WASI sandbox*
*Researched: 2026-04-20*
