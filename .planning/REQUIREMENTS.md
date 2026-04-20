# Requirements: WAVS Agent Runtime

**Defined:** 2026-04-20
**Core Value:** Developers can write an autonomous LLM agent in ~30 lines of Rust, compile it to WASM, deploy it as a WAVS service, and have it reason + act on triggers with full sandbox and cryptographic trust guarantees.

## v2.0 Requirements

Requirements for agent runtime milestone. Each maps to roadmap phases.

### WASI Compatibility (rig-core fork)

- [ ] **FORK-01**: rig-core compiles to wasm32-wasip2 with reqwest made optional behind a feature flag
- [ ] **FORK-02**: tokio `rt` feature removed; `tokio::sync::watch` replaced with `futures::channel` equivalent
- [ ] **FORK-03**: cfg detection unified — `WasmCompatSend`/`WasmBoxedFuture` use `target_family = "wasm"` consistently across all modules
- [ ] **FORK-04**: SSE module dead zones on wasip2 fixed (both cfg branches fire correctly)
- [ ] **FORK-05**: Fork compiles cleanly with `cargo build --target wasm32-wasip2` on a minimal test component

### Integration Library (wavs-rig crate)

- [ ] **RIG-01**: `WasiHttpClient` implements rig's `HttpClientExt` trait over `wasi:http/outgoing-handler`, routing all LLM API calls through the WASM sandbox
- [ ] **RIG-02**: Built-in WAVS tools implement rig's `Tool` trait: KvGetTool, KvSetTool, HttpFetchTool, EvmQueryTool, LogTool — each with typed args/output and JSON Schema definitions
- [ ] **RIG-03**: `WavsMemory` provides KV-backed conversation history with append, retrieve, and token budget truncation
- [ ] **RIG-04**: `WavsAgent` trait with `run_agent` shim bridges rig's agent loop to WASI component entry point via `wstd::runtime::block_on`
- [ ] **RIG-05**: Startup validation detects `AllowedHostPermission::None` and returns a clear error instead of silent HTTP trap failure

### Example & End-to-End

- [ ] **E2E-01**: Example agent component (~30 lines of domain logic) demonstrates full agent loop: trigger → LLM reasoning → tool use → structured result
- [ ] **E2E-02**: Agent deployed and executed end-to-end on a live WAVS node (trigger fires, agent reasons, result returned)
- [ ] **E2E-03**: `service.json` uses `AllowedHostPermission::Only(["api.anthropic.com"])` demonstrating sandboxed LLM access

## v3.0 Requirements

Deferred to future milestones. Tracked but not in current roadmap.

### Runtime Extensions

- **CONT-01**: Agent execution mode — `Continue` variant in WIT return type for multi-step agents that exceed single-invocation limits
- **RPC-01**: Service-to-service calls — `call-service` host function for inter-component composition
- **TOOL-01**: Structured tool abstraction in WIT with JSON Schema discovery

### App Integration

- **APP-01**: Agent-first workflow builder with template gallery and intent-driven config
- **APP-02**: Agent observability — reasoning timeline, live execution view, cost tracking

### Advanced Memory

- **MEM-01**: Fact store — key-value with metadata (source, confidence, timestamp, expiry)
- **MEM-02**: Embedding index — vector storage via KV, nearest-neighbor via external API

## Out of Scope

| Feature | Reason |
|---------|--------|
| Streaming LLM responses | WASI is single-threaded; no SSE consumer support |
| Concurrent tool execution | Requires threading unavailable in WASI sandbox |
| Multi-provider in single component | One provider per deployment via AllowedHostPermission is the security model |
| Agent-to-agent communication | Requires service-to-service RPC (v3.0) |
| Custom tool marketplace | Premature; establish patterns first |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FORK-01 | — | Pending |
| FORK-02 | — | Pending |
| FORK-03 | — | Pending |
| FORK-04 | — | Pending |
| FORK-05 | — | Pending |
| RIG-01 | — | Pending |
| RIG-02 | — | Pending |
| RIG-03 | — | Pending |
| RIG-04 | — | Pending |
| RIG-05 | — | Pending |
| E2E-01 | — | Pending |
| E2E-02 | — | Pending |
| E2E-03 | — | Pending |

**Coverage:**
- v2.0 requirements: 13 total
- Mapped to phases: 0
- Unmapped: 13 ⚠️

---
*Requirements defined: 2026-04-20*
*Last updated: 2026-04-20 after initial definition*
