# Project Research Summary

**Project:** WAVS Platform Extensions — WIT-to-schema, MCP Execution Interface, OCI Distribution
**Domain:** WASM component execution platform with AI agent integration and decentralized validation
**Researched:** 2026-03-24
**Confidence:** HIGH

## Executive Summary

This project adds three tightly coupled capabilities to the existing WAVS platform: (1) WIT-to-JSON-Schema tooling that introspects compiled WASM binary exports, (2) an MCP execution interface that exposes deployed WAVS services as callable AI agent tools across three trust tiers (result-only, signed-result, on-chain), and (3) OCI component distribution that lets services reference components hosted in OCI registries like `ghcr.io`. Research confirms all three are buildable using well-established crates — `wit-component`, `wit-parser`, `oci-client`, and `oci-wasm` — and that the existing WAVS architecture (`wavs-mcp`, dispatcher, engine, `WkgClient`) provides clean extension points for each feature without requiring major rewrites.

The recommended approach is to build in dependency order: OCI pull first (isolated change to `wkg.rs`), then WIT-to-schema (new `wit_schema.rs` module in engine + HTTP endpoint), then MCP execution interface (extends existing `wavs-mcp` server, depends on schema output). The key architectural insight from research is that WAVS operator components all share a single fixed `run(trigger-action)` export, so MCP tool `inputSchema` generation should be driven by the service definition's trigger type — not by WIT introspection of the component binary. WIT introspection is most valuable as a developer tool and for future custom-world components.

The primary risks are: (1) WIT variant and `u128` types do not map cleanly to JSON Schema — failing to handle this corrupts MCP tool schemas and causes agents to produce invalid inputs; (2) the trust tier mechanism must be a required parameter with unambiguous enum values to prevent agents from silently choosing on-chain submission when result-only was intended; (3) MCP stdio transport blocks on long-running components, requiring a hard 25-second MCP-layer timeout distinct from the engine's `time_limit_seconds`. All three risks have known mitigations and are avoidable if addressed in the correct phase.

---

## Key Findings

### Recommended Stack

The existing WAVS workspace already contains most of what is needed. The new additions are minimal and well-justified. For WIT introspection: `wit-component 0.245` (decodes compiled `.wasm` binary → `Resolve`) and `wit-parser 0.245` (traverses the decoded type tree) are the correct pair — `wit-parser` alone only handles `.wit` text files, not compiled binaries. `wasmparser 0.245` is the same version used by Wassette's `component2json`. All three are co-versioned in the `wasm-tools` monorepo; mixing minor versions breaks compilation. For OCI distribution: `oci-client 0.16` and `oci-wasm 0.4` are the correct pair — Bytecode Alliance and Wassette both use this combination. These crates are not yet in the workspace. The existing `wasm-pkg-client` routes by Warg namespace and cannot handle raw `oci://` URIs. For MCP execution: no new crates needed — the existing `rmcp 0.1` `ServerHandler` is fully dynamic and supports adding execution tools to the same server instance.

**Core technologies:**
- `wit-component 0.245` + `wit-parser 0.245`: Decode compiled WASM binary → structured WIT type tree — authoritative path confirmed by Wassette source and `oci-wasm` internals
- `wasmparser 0.245`: Low-level WASM binary parsing substrate, same version used by Wassette's `component2json`
- `oci-client 0.16` + `oci-wasm 0.4`: OCI registry pull for raw `oci://` URIs, bypassing wkg namespace resolution — Bytecode Alliance canonical implementation
- `rmcp 0.1` (existing): Dynamic MCP tool registration; no new MCP crates needed
- `wasmtime 42.0.1` (existing): `Component::component_type()` pre-instantiation introspection API stable since v28

**Critical version constraint:** `oci-wasm 0.4.0` depends on `wit-component 0.244` and `wit-parser 0.244`. If workspace uses `0.245`, Cargo will compile both. Needs a `cargo tree` check at implementation time; prefer aligning to 0.245 and letting Cargo unify.

### Expected Features

**Must have (table stakes):**
- Extract exported function signatures from compiled WASM binary — unblocks MCP `inputSchema` generation
- Map WIT primitive and record types to JSON Schema — without this, tool schemas are unusable
- Map WIT enum and variant types to JSON Schema with proper discriminators — required for `trigger-data` type
- Emit valid JSON Schema (draft-07+) with `inputSchema` and `outputSchema` per tool
- MCP `tools/list` populated dynamically from deployed services — agents cannot discover tools without this
- MCP `tools/call` handler that routes through existing WAVS engine — core execution path
- Trust tier as required enum parameter (`result_only` / `signed_result` / `on_chain`) on every execution tool
- OCI pull with SHA256 digest verification at deploy time — security non-negotiable
- Disk cache keyed by digest — avoid re-pulling identical content

**Should have (differentiators):**
- `outputSchema` population from WIT return types — MCP 2025-06-18 spec supports this; WAVS can be ahead of Wassette
- Signed result envelope in Tier 2 with operator public key + signature — the core WAVS differentiator over Wassette
- WIT doc comments embedded as JSON Schema `description` fields — richer MCP tool descriptions
- `notifications/tools/list_changed` when services deploy/remove — agents should not need to reconnect
- Digest pinning enforcement in `service.json` — warn or fail on mutable tags without `@sha256:` pin
- Structured OCI error codes (pull_failed / digest_mismatch / registry_unavailable)
- Auto-generated description from function name when no WIT docs present
- Content-addressed local storage for pulled OCI components

**Defer (v2+):**
- Trust Tier 3 on-chain submission — high complexity, high latency; design as documented follow-on after Tier 2 ships
- WIT resource type support — Wassette issue #601 unresolved; resource types are not yet common in WAVS components
- Authenticated OCI pull / private registry UI — most initial components are public; add auth when first enterprise user needs it
- Multi-operator Tier 2 quorum signing — single-operator signed result ships first; quorum is a follow-on
- OCI push/publish tooling — explicitly out of scope for this milestone; use `wkg oci push`

### Architecture Approach

The three features integrate into WAVS through existing extension points with minimal invasive changes. The key integration sites are: `packages/engine/src/common/` (new `wit_schema.rs` module for WIT introspection), `packages/utils/src/wkg.rs` (modified `WkgClient::new()` to detect OCI domains and emit `type = "oci"` config), `packages/wavs/src/dispatcher.rs` (new `execute_direct()` method bypassing TriggerManager for Tier 1/2), two new HTTP handlers in `packages/wavs/src/http/handlers/service/` (`execute.rs` and `schema.rs`), and `packages/wavs-mcp/src/` (new `execution.rs` module + modifications to `server.rs` for dynamic tool listing). All existing subsystem channels, the CA store, and existing management tools remain untouched. OCI pull integrates through the existing `ComponentSource::Registry` path — `WkgClient` already has the right API; only the backend config is missing.

**Major components and responsibilities:**
1. `packages/engine/src/common/wit_schema.rs` (NEW) — Pure function `component_bytes_to_schema(bytes) -> Vec<ToolSchema>` using wasmtime + wit-component; no service context needed; operates on raw bytes cached by digest
2. `packages/wavs/src/http/handlers/service/execute.rs` (NEW) — `POST /dev/execute/{service_id}/{workflow_id}` endpoint; dev-endpoints-gated; routes to `Dispatcher::execute_direct()`
3. `packages/wavs/src/http/handlers/service/schema.rs` (NEW) — `GET /dev/components/{digest}/schema`; retrieves bytes from CA store, calls wit_schema module
4. `packages/wavs-mcp/src/execution.rs` (NEW) — Trust tier logic; builds `TriggerAction` from MCP input; signs result for Tier 2; queues on-chain submission for Tier 3
5. `packages/utils/src/wkg.rs` (MODIFIED) — Detect OCI registry domains (ghcr.io, docker.io, etc.) and configure `type = "oci"` wasm-pkg-client backend instead of warg
6. `packages/wavs-mcp/src/server.rs` (MODIFIED) — Dynamic `list_tools()` calls `GET /services`, emits one `run_{service}_{workflow}` tool per workflow with trigger-type-derived `inputSchema`

### Critical Pitfalls

1. **Trust tier confusion causes agents to trigger on-chain transactions when result-only was intended** — Make trust tier a required enum parameter (not optional with default) on every execution tool; use names `result_only`, `signed_result`, `on_chain`; add `destructive: true` annotation on `on_chain`; test by asking an LLM to select a tier for 10 representative tasks before shipping

2. **WIT variant types and `u128` produce broken JSON Schemas** — WIT `variant` (discriminated union) with 7 cases like `trigger-data` cannot be mechanically auto-generated; `u128` is not natively supported by `serde_json`; decouple agent-facing MCP schema from internal WIT types; for variants use `oneOf` with required `tag` discriminator; encode `u128` as string; add `for_llm: bool` parameter on schema tool that enables friendly mappings

3. **MCP stdio transport blocks on long-running WASM components** — Set a hard MCP-layer timeout of 25 seconds (below the common 30-second MCP client default) independent of the engine's `time_limit_seconds`; expose as `--mcp-exec-timeout-secs` flag; test with a component that sleeps 31 seconds

4. **OCI pull without digest pinning enables supply chain attacks** — Require a digest field in `service.json` alongside any OCI URI; refuse to deploy without it; cache by digest not by tag; verify SHA256 of pulled bytes before loading into engine; warn on mutable tags

5. **Breaking the existing wavs-mcp management interface when adding execution tools** — Use `wavs_run_` prefix for execution tools (not `wavs_` or `wavs_exec_`); add `--exec-enabled` flag to opt in; write a `list_tools` diff regression test that verifies no existing management tool schema changes after adding execution tools

---

## Implications for Roadmap

Based on research findings, the dependency graph is clear: OCI pull is independent, WIT-to-schema depends on nothing new, and MCP execution depends on both. Build in three phases with MCP execution sub-ordered by trust tier complexity.

### Phase 1: OCI Component Pull

**Rationale:** Fully independent of the other two features. The change is surgically small (modify `WkgClient::new()` in one file). Completing this first means all subsequent testing of MCP execution and WIT schema tools can use OCI-hosted components from `ghcr.io/microsoft/` rather than requiring local file paths — dramatically accelerating the test feedback loop for phases 2 and 3.

**Delivers:** Service definitions can reference OCI-hosted WASM components via `Registry { domain: "ghcr.io", ... }`; components are pulled at deploy time, verified by digest, and cached by SHA256 for subsequent deploys.

**Addresses:** OCI table stakes (pull, digest verification, disk cache, anonymous auth, structured error codes), digest pinning enforcement

**Avoids:** Supply chain attack via mutable tags (Pitfall 5); separate OCI cache layer anti-pattern; blocking node startup for slow pulls

**Research flag:** Standard patterns — OCI pull via `oci-client` + `oci-wasm` is well-documented; Wassette and `wasm-pkg-client` provide reference implementations. No additional research phase needed.

---

### Phase 2: WIT-to-Schema Tooling

**Rationale:** Must ship before MCP execution interface because it generates the `inputSchema` and `outputSchema` fields for execution tools. Pure addition — no existing behavior changes. Can ship as a standalone developer tool (`wavs_get_component_schema` MCP tool + `GET /dev/components/{digest}/schema` endpoint) with immediate value even before MCP execution tools exist.

**Delivers:** `component_bytes_to_schema()` function in engine; HTTP schema endpoint; MCP `wavs_get_component_schema` tool; type mapping covering all WIT primitives, records, enums, variants, options, results, tuples, and lists; schema cached by component digest

**Addresses:** WIT primitive/record/enum/variant type mapping, output schema generation, schema caching, CLI subcommand ergonomics

**Avoids:** Using `.wit` text files instead of compiled binary (Pitfall integration gotcha); WIT introspection at execution time (Architecture anti-pattern 2); overly permissive schemas (`additionalProperties: true`); raw `list<u8>` as byte array (LLMs cannot reliably produce byte arrays)

**Research flag:** Needs attention during planning for `u128` and variant type edge cases. The `trigger-data` variant with 7 cases is a known hard case — plan the `oneOf` + discriminator convention before implementing. Verify `Component::component_type().exports(engine)` method signature for wasmtime 42.0.1 specifically (Wassette was built against an earlier version).

---

### Phase 3: MCP Execution Interface — Tier 1 (Result Only)

**Rationale:** Build the simplest trust tier first. `ResultOnly` requires no signing infrastructure and no blockchain coordination — it is direct engine execution returning raw output. This establishes the complete MCP execution data flow (tool listing, tool calling, error propagation, timeout handling) before adding trust-tier complexity. Depends on Phase 2 for `inputSchema`; can use static trigger-type schemas if Phase 2 is not yet complete.

**Delivers:** Dynamic `tools/list` populated from deployed services; `run_{service}_{workflow}` tools with trust tier as required parameter; `POST /dev/execute/{service_id}/{workflow_id}` endpoint; 25-second MCP-layer timeout; `--exec-enabled` flag; `wavs_run_` naming prefix; error propagation as structured MCP errors

**Addresses:** `tools/list` dynamic population, `tools/call` handler, naming convention, error propagation, `notifications/tools/list_changed`, `--exec-enabled` opt-in

**Avoids:** Blocking the existing management tools interface (Pitfall 4 — `list_tools` diff regression test); separate MCP server binary (Architecture anti-pattern 1); MCP stdio blocking on long-running components (Pitfall 3); trust tier as optional parameter (Pitfall 1)

**Research flag:** Low — the execution data flow is well-mapped by architecture research. Implement `list_tools` caching (5-second TTL) before shipping to prevent `GET /services` on every tool list call.

---

### Phase 4: MCP Execution Interface — Tier 2 (Signed Result)

**Rationale:** The key WAVS differentiator over Wassette. Builds directly on Phase 3's execution path by adding operator signing after engine execution. The signing infrastructure already exists in `packages/types/src/signing.rs` — this phase wires it to the MCP execution response.

**Delivers:** Tier 2 execution returns `{ result, signature, signer }` — verifiable proof that this operator with this binary produced this output; single-operator signed result (quorum deferred)

**Addresses:** Signed result envelope, operator public key in response, trust tier semantics clearly differentiated from Tier 1

**Avoids:** Ad-hoc signing bypassing existing signing infrastructure; conflating Tier 2 with Tier 3 in agent descriptions

**Research flag:** Needs one targeted investigation before implementation: verify whether the existing `SignatureKind` infrastructure in `packages/types/src/signing.rs` supports ad-hoc single-operator signing without aggregation. The existing signing path is driven by the Aggregator collecting multiple signatures — confirm there is a direct signing path for single-operator use.

---

### Phase 5: MCP Execution Interface — Tier 3 (On-Chain Submission) [Deferred]

**Rationale:** Highest complexity tier. Routes through the full existing Aggregator + Submission pipeline, same as normal trigger execution. Deferred because: (1) it adds blockchain coordination overhead to a latency-sensitive MCP path; (2) `OnChain` is inherently async (block times 2-12s), and MCP async patterns (Tasks primitive) are not yet widely supported by MCP clients; (3) Tier 2 delivers the core WAVS positioning and ships faster.

**Delivers:** Tier 3 execution queues through existing aggregator + submission path; returns `{ tx_hash }` or a job ID for polling; permanent on-chain audit record of agent tool invocations

**Addresses:** On-chain submission use cases, permanent auditability

**Avoids:** Synchronous blocking until on-chain confirmation (Pitfall 3 — return async with job ID); granting `AllowedHostPermission::All` as default

**Research flag:** Needs research phase before planning — specifically: how to expose async Tier 3 results through MCP's synchronous stdio transport; whether to use the MCP Tasks primitive or a polling resource URI. Also: verify that a single-operator test node produces a valid on-chain result through the existing aggregator path.

---

### Phase Ordering Rationale

- **OCI first** because it is independent, low-risk, and enables the rest of testing to use real OCI-hosted components
- **WIT-to-schema second** because MCP execution tools need `inputSchema` and `outputSchema`; without schema, execution tools are blind
- **MCP Tier 1 third** because it establishes the complete execution data flow before adding signing/chain complexity
- **MCP Tier 2 fourth** because it is the key differentiator; the signing path exists and just needs to be wired in
- **MCP Tier 3 deferred** because its async nature and blockchain coordination require a separate design decision about MCP Tasks or polling patterns

This ordering matches all three research files' build-order recommendations. It also front-loads the independent/lower-risk work and defers the highest-complexity coordination problem (async on-chain submission over synchronous MCP transport) until patterns are clear.

### Research Flags

Phases needing deeper research during planning:
- **Phase 2 (WIT-to-schema):** WIT variant and `u128` edge cases need a concrete convention decision before any code is written. Verify wasmtime 42.0.1 `Component::component_type()` method signature — Wassette used an older wasmtime version and the API may have changed.
- **Phase 4 (Tier 2 signing):** Verify whether `packages/types/src/signing.rs` supports single-operator ad-hoc signing without the Aggregator. If not, a thin signing wrapper is needed.
- **Phase 5 (Tier 3, deferred):** Full research phase required before planning. Async execution over MCP stdio is the central unsolved design problem.

Phases with standard patterns (skip research-phase):
- **Phase 1 (OCI pull):** Bytecode Alliance provides `oci-wasm` reference implementation; Wassette and `wasm-pkg-client` confirm the approach. `WkgClient` modification is straightforward.
- **Phase 3 (MCP Tier 1):** Architecture research fully traced the execution data flow. The `simulate_trigger` existing tool and `EngineCommand::ExecuteOperator` provide a clear integration pattern.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Critical crate versions confirmed against crates.io, Wassette source, and docs.rs. Version compatibility between `oci-wasm 0.4` and `wit-component 0.244/0.245` is the one unresolved detail; needs `cargo tree` check at implementation time. |
| Features | HIGH | Feature landscape verified against Wassette source, MCP spec 2025-06-18, and CNCF WASM OCI spec. MVP priority order is opinionated and defensible. |
| Architecture | HIGH | Based on direct codebase inspection of all affected packages. Existing extension points (dispatcher, WkgClient, wavs-mcp ServerHandler) are clearly identified. |
| Pitfalls | HIGH | Critical pitfalls are codebase-verified (server.rs, events.wit, signing path) plus external evidence (MCP issue tracker, supply chain attack documentation). |

**Overall confidence:** HIGH

### Gaps to Address

- **`wit-component` vs `oci-wasm` version alignment:** `oci-wasm 0.4.0` pins `wit-component = "0.244.0"` while workspace will use `0.245`. Run `cargo tree` at Phase 2 implementation start; if Cargo cannot unify, pin workspace to `0.244`.
- **Single-operator signing path:** The existing signing infrastructure is driven by multi-operator aggregation. Before implementing Tier 2, confirm whether `alloy-signer` HD key derivation can produce a standalone ECDSA signature without routing through the Aggregator subsystem.
- **wasmtime 42 `Component::component_type()` API:** Wassette's `component2json` was built against an earlier wasmtime version. Validate the exact method signature and export iterator API for wasmtime 42.0.1 before writing `wit_schema.rs`.
- **`list_tools` performance at scale:** Calling `GET /services` on every `list_tools` invocation will slow as service count grows. A 5-second TTL in-memory cache in `WavsMcpServer` is the planned mitigation — design this before Phase 3 implementation, not after.
- **Tier 3 async design:** Async on-chain submission over MCP's synchronous stdio transport is unresolved. Candidate patterns (job ID + polling resource URI vs. MCP Tasks primitive) need evaluation before Phase 5 can be planned.

---

## Sources

### Primary (HIGH confidence)
- Wassette `component2json` Cargo.toml — confirmed `wasmparser 0.245`, `oci-client 0.16`, `oci-wasm 0.4`
- `wit-component` docs.rs (v0.245.1) — `decode()` function confirmed
- `wit-parser` docs.rs (v0.245.1) — struct inventory confirmed
- `oci-wasm` GitHub Cargo.toml (v0.4.0) — `oci-client 0.16`, `wit-component 0.244.0` confirmed
- `oci-client` docs.rs (v0.16.1) — pull methods and `RegistryAuth` confirmed
- MCP Tools Specification 2025-06-18 — `inputSchema`, `outputSchema`, `notifications/tools/list_changed`
- CNCF TAG Runtime WASM OCI Artifact spec — media types confirmed
- Direct codebase inspection: `packages/wavs-mcp/src/server.rs`, `packages/engine/src/common/base_engine.rs`, `packages/utils/src/wkg.rs`, `packages/types/src/service.rs`, `packages/wavs/src/dispatcher.rs`, `wit-definitions/operator/wit/operator.wit`, `wit-definitions/types/wit/events.wit`, `wit-definitions/types/wit/core.wit`

### Secondary (MEDIUM confidence)
- Bytecode Alliance component model distribution docs — OCI artifact format
- Microsoft OCI + WASM blog — media types `application/vnd.wasm.config.v0+json` and `application/wasm`
- MCP Long-Running Operations issue #1391 — stdio blocking documented failure mode
- OWASP MCP Top 10 — security posture for execution interface
- Tool name collision community reports (Cursor forum) — naming convention rationale

### Tertiary (LOW confidence — needs validation)
- `list_tools` caching TTL recommendation (5 seconds) — inferred from general MCP client behavior patterns; not benchmarked against WAVS specifically
- Tier 3 async design options — candidate patterns identified but not validated against current MCP client support

---
*Research completed: 2026-03-24*
*Ready for roadmap: yes*
