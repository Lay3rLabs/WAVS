# Phase 18: wavs-rig Integration Crate - Context

**Gathered:** 2026-04-20
**Status:** Ready for planning
**Mode:** Auto-generated (autonomous mode)

<domain>
## Phase Boundary

`packages/wavs-rig` is a library crate bridging rig-wasi (the fork from Phase 17) into WAVS WASI components. It provides: (1) WasiHttpClient implementing rig's HttpClientExt over wasi:http, (2) five built-in tool impls (KvGet, KvSet, HttpFetch, EvmQuery, Log), (3) KV-backed conversation memory with token budget truncation, (4) a WavsAgent trait with run_agent async shim, (5) startup validation for AllowedHostPermission.

</domain>

<decisions>
## Implementation Decisions

### HTTP Transport
- WasiHttpClient wraps wstd::http::Client (already used in packages/wasi-utils/src/http.rs) to implement rig's HttpClientExt trait
- Request/response mapping: convert rig's http types ↔ wstd::http types
- Auth headers (API keys) passed through from agent config, not hardcoded

### Built-in Tools
- Each tool is a separate struct implementing rig's Tool trait
- KvGetTool/KvSetTool use wasi:keyvalue host bindings (already available in WAVS engine)
- HttpFetchTool uses WasiHttpClient for external HTTP calls
- EvmQueryTool uses existing wavs-wasi-utils EVM helpers
- LogTool writes to wasi:logging
- All tools have typed args/output with serde + JSON Schema via schemars

### Conversation Memory
- WavsMemory stores messages as JSON in wasi:keyvalue under a conversation key prefix
- Append: push new message to list
- Retrieve: load all messages for conversation
- Truncation: drop oldest messages when estimated token count exceeds budget
- Token estimation: simple char-count / 4 heuristic (no tokenizer dep in WASM)

### Agent Entry Point
- WavsAgent trait with async fn run(trigger_data) -> Result<AgentOutput>
- run_agent shim wraps the trait call inside wstd::runtime::block_on
- Single block_on call — no nested async runtimes (prevents deadlock)
- Agent output is structured (serde serializable) for WAVS result submission

### Startup Validation (RIG-05)
- Before agent execution, check if HTTP outgoing is available via wasi:http capability probe
- If AllowedHostPermission::None → return clear error string, not silent WASI trap
- Error message: "WAVS agent requires HTTP access — set AllowedHostPermission to All or Only"

### Claude's Discretion
- Internal module organization within packages/wavs-rig
- Error types and error handling patterns
- Any additional utility functions needed for the bridge
- Token budget default value
- Whether to re-export rig types or require consumers to depend on rig-wasi directly

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `packages/wasi-utils/src/http.rs` — wstd::http::Client helpers (fetch_bytes, fetch_json, fetch_string)
- `packages/wasi-utils/src/evm/` — EVM query helpers for EvmQueryTool
- `packages/rig-wasi/` — Phase 17 fork with HttpClientExt trait to implement
- `examples/components/kv-store/` — KV usage patterns in WASI components

### Established Patterns
- WASI components use wstd::runtime::block_on for async entry
- HTTP via wstd::http::Client (not reqwest on WASM)
- KV via wasi:keyvalue host bindings
- Components implement wavs world interfaces
- Components are cdylib crates

### Integration Points
- rig-wasi's HttpClientExt trait (packages/rig-wasi/src/http_client/mod.rs)
- rig-wasi's Tool trait for built-in tools
- WAVS engine AllowedHostPermission (packages/types/src/service.rs)
- wavs-wasi-utils helpers (packages/wasi-utils/)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Follow existing WAVS patterns.

</specifics>

<deferred>
## Deferred Ideas

- Agent continuation mode (CONT-01) — v3.0
- Service-to-service calls (RPC-01) — v3.0
- Structured tool abstraction in WIT (TOOL-01) — v3.0
- Embedding index / fact store (MEM-01, MEM-02) — v3.0

</deferred>

---

*Phase: 18-wavs-rig-integration-crate*
*Context gathered: 2026-04-20 via autonomous smart discuss*
