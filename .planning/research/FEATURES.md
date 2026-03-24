# Feature Landscape

**Domain:** WASM component execution platform with MCP execution interface, WIT-to-schema tooling, and OCI distribution
**Researched:** 2026-03-24
**Milestone scope:** WIT-to-schema tooling, end-user MCP execution interface (three trust tiers), OCI component pull

---

## Capability Area 1: WIT-to-Schema Tooling

### What Wassette Does

Wassette's `component2json` crate extracts typed interface information from a compiled WASM component binary. The approach has two parts:

1. **Static annotation:** `wit-docs-inject` embeds WIT documentation as a `package-docs` custom section in the WASM binary at build time. This carries the author's human-readable descriptions of functions and parameters.
2. **Runtime extraction:** `component2json` (or the `wassette inspect` subcommand) decodes the component's type section using `wasmparser`/`wit-component`, reads the embedded docs section, and emits a JSON Schema describing each exported function's input and output types.

The Bytecode Alliance community has opened issue #579 to consider upstreaming `component2json` into `wit-bindgen`. Status as of research date: open/unresolved — the canonical upstream home has not been decided.

Alternative architectural discussion (issue #432): building `component2json` on top of WAVE (an encoding scheme for WIT values) rather than raw wasmparser traversal. WAVE provides a more principled round-trip between WIT values and JSON.

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Extract exported function signatures from compiled binary | Required to auto-generate MCP tool `inputSchema` | Medium | wasmparser + wit-component crates handle binary decoding; mapping WIT types to JSON Schema types is the work |
| Map WIT primitive types to JSON Schema | Without this, tool descriptions are meaningless | Low-Medium | `s32/s64/u32/u64` → `integer`, `f32/f64` → `number`, `string` → `string`, `bool` → `boolean`, `option<T>` → nullable, `result<T,E>` → needs convention |
| Map WIT record types to JSON Schema objects | Records are the common parameter-passing type | Medium | Recursive traversal of nested record fields |
| Map WIT enum/variant types to JSON Schema | Variant types are common in idiomatic WIT | Medium | Enum → `enum` array; variant with payloads needs `oneOf` |
| Emit both `inputSchema` and `outputSchema` | MCP spec supports both; output schema enables client validation | Low | `outputSchema` is optional in MCP but expected by quality tooling |
| Produce valid JSON Schema (draft-07 or later) | MCP `inputSchema` field must be valid JSON Schema | Low | Use `$schema` header |
| CLI subcommand (`wavs wit-schema <component.wasm>`) | Developer ergonomics; inspection without running a service | Low | Equivalent to `wassette inspect` |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Embed WIT doc comments as JSON Schema `description` fields | Richer MCP tool descriptions without manual annotation | Medium | Requires build-time `wit-docs-inject` step OR compile-time embedding via proc-macro in WIT bindgen |
| Auto-generate description from function name if no docs present | Graceful degradation; avoids blank descriptions in MCP | Low | Heuristic: `compute_hash` → "Compute hash" |
| Output both human-readable and machine-readable formats | `--format json|markdown` for schema dump | Low | Markdown useful for documentation generation |
| Schema caching per component SHA256 digest | Avoid re-parsing unchanged binaries | Low | Cache keyed by content hash; fits naturally with OCI digest |
| `outputSchema` population from WIT return types | MCP 2025-06-18 spec added `outputSchema`; Wassette lags here | Low | WAVS can be ahead of Wassette on spec compliance |
| WIT resource type support | Issue #601 in Wassette is open/unresolved | High | WIT resources are stateful handles; MCP has no direct equivalent — needs convention |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Requiring developers to annotate structs with `#[derive(JsonSchema)]` | Forces runtime Rust dependency on `schemars`; breaks the WIT-first model | Extract schema entirely from WIT type information in the compiled binary |
| Generating overly permissive schemas (`"type": "object", "additionalProperties": true`) | LLMs use schema to constrain calls; loose schemas produce bad results | Use `additionalProperties: false` on all generated records |
| Separate per-language annotation syntax | WIT is the source of truth; language-specific schema annotations fragment the ecosystem | Invest in WIT doc comment embedding over language-level workarounds |
| Upstreaming before shipping | Waiting for Bytecode Alliance to resolve issue #579 delays the milestone | Build WAVS-specific implementation now; make it easy to swap for upstream when it lands |

### Dependencies

- Depends on: `wasmparser`, `wit-component` crates (existing BA toolchain)
- Enables: MCP execution interface (capability area 2) — auto-generated `inputSchema`/`outputSchema` per tool
- No dependency on OCI (capability area 3) — schema generation works on local `.wasm` files

---

## Capability Area 2: MCP Execution Interface (Three Trust Tiers)

### What Wassette Does

Wassette exposes each WASM component's exported functions as MCP tools directly. One tool per exported function. The trust model is flat: Wasmtime sandbox isolation, deny-by-default host permissions (network, filesystem, env vars), and interactive permission approval during agent calls. No cryptographic result signing, no multi-operator consensus, no blockchain submission. Single-machine trust assumption. Agent cannot request a stronger guarantee.

### What WAVS Adds

WAVS already has: cryptographic operator signatures, multi-operator aggregation, EVM/Cosmos on-chain submission, and an existing `wavs-mcp` management server. The MCP execution interface exposes deployed services as callable tools and adds the trust tier as an agent-controlled parameter per call.

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| `tools/list` response populated from deployed services | Without this, agents cannot discover what tools are available | Medium | Query the service registry; one tool entry per deployed component/workflow |
| `tools/call` handler that executes the component | Core execution path — without this the interface is a stub | High | Route through existing WAVS engine; adapt request/response to MCP content types |
| Tool name derived from service name + component export | Stable, predictable naming for agents | Low | Convention: `{service_name}/{export_name}` or flat with collision avoidance |
| `inputSchema` auto-populated from WIT-to-schema output | Agents need schema to construct valid calls | Low (given CA1) | Depends on WIT-to-schema being built first |
| `description` populated from WIT docs or heuristic | Agents use description for tool selection | Low (given CA1) | Falls back to function name if no docs |
| `notifications/tools/list_changed` when services are deployed/removed | Agents should not need to reconnect after deploy | Low | Emit on service registration/deregistration events |
| Error propagation to MCP `isError: true` result | Agents must know if execution failed | Low | Map WAVS engine errors to MCP error content |
| Extend `wavs-mcp` (not a new server) | Single MCP server for management + execution reduces user friction | Medium | Requires merging two handler sets in the existing `wavs-mcp` process |

### Differentiators — Trust Tiers

The trust tier is the core WAVS differentiator. Exposed as a tool call parameter (agent-controlled per call) or as part of the tool name variant. Three tiers:

| Tier | What Agent Receives | Complexity | Notes |
|------|---------------------|------------|-------|
| **Tier 1: Result only** | Raw execution output, no proof | Low | Identical to Wassette behavior; sandbox isolation only |
| **Tier 2: Result + operator signature** | Output + ECDSA/BLS signature from operator(s) proving what was executed | Medium | Leverages existing WAVS operator signing; wrap result in signed envelope before returning MCP content |
| **Tier 3: On-chain submission** | Transaction hash proving result was anchored on-chain | High | Triggers full WAVS submission pipeline; agent waits for confirmation |

**Trust tier exposure patterns** (choose one):

- **Explicit parameter:** Single tool with `trust_tier: 1|2|3` in `inputSchema`. Simple, one tool per component. Forces schema to include a non-domain parameter.
- **Parallel tools:** Three tool entries per component, named `{name}`, `{name}_signed`, `{name}_onchain`. Clean separation. Triples the `tools/list` response size.
- **Tool annotation:** Use MCP `annotations` field to advertise available tiers; agent selects tier via separate mechanism. Most future-proof but requires agent-side understanding.

Recommended: explicit parameter approach for v1. It is the lowest surface area and does not require agents to understand WAVS-specific naming conventions.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Trust tier as explicit `inputSchema` parameter | Clean agent interface; single tool per component | Low-Medium | Adds one field to every generated schema |
| Signed result envelope in Tier 2 | Verifiable proof that THIS operator with THIS binary produced THIS output | Medium | Return operator public key + signature + raw result as structured MCP content |
| Async on-chain confirmation in Tier 3 | Permanent, auditable record of agent tool invocations | High | Requires either synchronous wait with timeout or async resource subscription pattern |
| Per-call timeout configuration | Long-running components must not block MCP client indefinitely | Low | Expose as optional `inputSchema` field with node-level max |
| Multi-operator result agreement in Tier 2 | Quorum-based signing gives stronger guarantee than single operator | High | Use existing aggregator; adds latency |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Separate MCP server binary for execution | Doubles operational complexity; users already have `wavs-mcp` running | Extend existing `wavs-mcp` with execution handler set |
| Tier 3 synchronous blocking until on-chain confirmation | Block times are 2-12s on most chains; MCP clients timeout | Return Tier 3 as async: immediately return a pending-status result with a resource URI the agent can poll |
| Exposing raw bytes in MCP content | Agents cannot use raw binary data | Always JSON-encode results; use text/content with structured data |
| Trust tier enforcement on the server side only | Single-operator attestation is meaningless if the agent cannot verify | Include the operator's public key and signature in the Tier 2 response; document verification procedure |
| Auto-registering every internal service as a tool | Internal infrastructure services should not be agent-callable | Use an explicit opt-in flag per service (e.g., `mcp_exposed: true` in service.json) |
| Designing for a single trust model | Forces all use cases onto on-chain overhead | The dial metaphor is the product — do not collapse tiers |

### Dependencies

- Depends on: WIT-to-schema (CA1) for `inputSchema`/`outputSchema` generation
- Depends on: existing WAVS engine, operator signing, submission pipeline (already built)
- Depends on: existing `wavs-mcp` management server (to extend, not replace)
- No hard dependency on OCI (CA3) — services can be deployed from local files

---

## Capability Area 3: OCI Component Pull

### What Wassette Does

Wassette loads components via `oci://` URIs at startup (e.g., `oci://ghcr.io/microsoft/time-server-js:latest`). The pull happens on startup, not on-demand. Wassette maintains 12 curated components in `ghcr.io/microsoft/` as a reference registry. No on-disk caching details are documented publicly. No publishing tooling is exposed (pulling only). No digest pinning requirements documented.

### Standard OCI Artifact Format for WASM

Established by CNCF TAG Runtime and implemented by Bytecode Alliance `wasm-pkg-tools`:

- **Config media type:** `application/vnd.wasm.config.v0+json`
- **Layer media type:** `application/wasm`
- **Manifest schema version:** 2 (OCI Image Manifest v1)
- **Architecture/OS fields:** `wasm` / `wasip2` (or `wasip1`)
- **Content addressing:** SHA256 digest per layer; `layerDigests` array in config links layers to the manifest
- **Verification:** Standard OCI digest verification; `wkg.lock` records SHA256 per pulled package for reproducibility
- **Registry compatibility:** Any OCI 1.1 compliant registry (ghcr.io, Docker Hub, private registries)

Rust implementation: `rust-oci-wasm` crate (Bytecode Alliance) wraps `oci-distribution` with WASM-specific config types. `wasm-pkg-tools` / `wkg` CLI provides the full push/pull workflow.

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Pull component from `oci://` URI at service deploy time | Core requirement per PROJECT.md; matches Wassette behavior | Medium | Use `rust-oci-wasm` or `oci-distribution` crate; not novel |
| SHA256 digest verification after pull | Prevents tampered binary from being loaded | Low | OCI spec provides digest in manifest; verify before write |
| Disk cache keyed by digest | Avoid re-pulling identical content across deploys | Low | Cache directory with digest-named files; dedup by hash |
| Support `ghcr.io` as primary registry | Wassette's component ecosystem lives there | Low | Standard OCI; no ghcr-specific logic needed |
| Anonymous pull for public components | Public components should not require auth | Low | `oci-distribution` supports unauthenticated pulls |
| `service.json` `oci://` URI format | Deploy-time declarative pull; consistent with Wassette convention | Low | Parse URI scheme; route to OCI pull vs local file path |
| Error on digest mismatch | Security-critical; fail loudly if content doesn't match declared hash | Low | Hard fail; log the mismatch details |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Digest pinning in `service.json` | `oci://ghcr.io/foo/bar@sha256:abc123` is reproducible; `:latest` is not | Low | Parse `@sha256:` suffix; require for production deploys; warn if only tag given |
| Authenticated pull via environment credential | Private registry support for enterprise components | Medium | Pass registry auth token via env var; `oci-distribution` supports this |
| WIT interface verification after pull | After pulling, decode the component and verify its exported interface matches what `service.json` declares | Medium | Use WIT-to-schema tooling (CA1); prevents deploying wrong component version |
| Content-addressed local storage | Store pulled components at `{cache_dir}/{sha256}` so identical components across services share disk | Low | Structural dedup; important for operators running many services |
| Pull progress reporting via WAVS node logs | Large components (>5MB) take time; operators need visibility | Low | Stream pull progress to tracing span |

### Anti-Features

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| OCI push/publish tooling in this milestone | Scope creep; PROJECT.md explicitly defers publishing | Pull-only; document that publishing uses `wkg oci push` or standard OCI tooling |
| Private registry UI in the desktop app | Desktop app is out of scope for this milestone | Store auth credentials in env or config file; no UI needed |
| Re-pulling on every service start | Wastes bandwidth; defeats content addressing | Cache-first: check digest cache before network |
| Custom OCI format (non-standard media types) | Breaks compatibility with Wassette's component ecosystem | Strictly follow CNCF spec: `application/vnd.wasm.config.v0+json` + `application/wasm` |
| Blocking node startup for slow pulls | Node is unavailable until all oci:// components download | Pull async at deploy time; queue execution until pull completes; do not block node boot |
| Treating tag-based URIs as stable | `:latest` is mutable; different content can appear at same tag | Warn on deploy if no digest is pinned; recommend `@sha256:` pinning |

### Dependencies

- No dependency on CA1 or CA2 for basic pull
- CA1 (WIT-to-schema) enables interface verification after pull (differentiator)
- Independent track per PROJECT.md — can be built in parallel with CA1/CA2

---

## Feature Dependencies

```
CA1 (WIT-to-schema) ──────────────────────────────── Required
        │
        └─→ CA2 (MCP execution interface)            Depends on CA1 for inputSchema/outputSchema

CA3 (OCI pull) ─────────────────────────────────── Independent
        │
        └─→ CA1 (optional: post-pull interface       Enhancement only; CA3 ships without CA1
                 verification)
```

Ordering: build CA1 first, then CA2, CA3 in parallel with or after CA1.

---

## MVP Recommendation

Prioritize in this order:

1. **WIT primitive + record type mapping to JSON Schema** — unblocks everything else; medium complexity; discrete deliverable
2. **MCP `tools/list` + `tools/call` in wavs-mcp** — connects schema to agent interface; Tier 1 execution first
3. **Trust Tier 2 (signed result)** — the key differentiator over Wassette; depends on Tier 1 working
4. **OCI pull with digest verification** — independent track; medium complexity; unblocks community component use
5. **WIT doc comment embedding** — quality-of-life for developers building components; deferred until core path works

Defer:
- **Trust Tier 3 (on-chain submission)** — high complexity, high latency; deliver as documented follow-on after Tier 2 ships
- **WIT resource type support** — Wassette issue #601 is open; this is a hard problem; defer until resource types are commonly used in WAVS components
- **Authenticated OCI pull** — most initial components are public; add auth when first enterprise user needs it
- **Multi-operator Tier 2 (quorum signing)** — single-operator signed result ships first; quorum is a follow-on

---

## Sources

- [Microsoft Wassette GitHub](https://github.com/microsoft/wassette)
- [Introducing Wassette — Microsoft Open Source Blog](https://opensource.microsoft.com/blog/2025/08/06/introducing-wassette-webassembly-based-tools-for-ai-agents/)
- [Wassette FAQ — limitations documentation](https://microsoft.github.io/wassette/latest/faq.html)
- [Wassette Rust Cookbook — component build process](https://microsoft.github.io/wassette/latest/cookbook/rust.html)
- [Wassette v0.3.4 release notes](https://github.com/microsoft/wassette/releases/tag/v0.3.4)
- [Wassette issue #579 — consider upstreaming component2json to wit-bindgen](https://github.com/microsoft/wassette/issues)
- [MCP Tools Specification 2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
- [CNCF TAG Runtime — Wasm OCI Artifact spec](https://tag-runtime.cncf.io/wgs/wasm/deliverables/wasm-oci-artifact/)
- [Bytecode Alliance wasm-pkg-tools](https://github.com/bytecodealliance/wasm-pkg-tools)
- [Bytecode Alliance rust-oci-wasm](https://github.com/bytecodealliance/rust-oci-wasm)
- [Distributing WASM components using OCI registries — Microsoft Open Source Blog](https://opensource.microsoft.com/blog/2024/09/25/distributing-webassembly-components-using-oci-registries/)
- [Bytecode Alliance component model distribution docs](https://component-model.bytecodealliance.org/composing-and-distributing/distributing.html)
- [Dynamic Tool Discovery — Speakeasy](https://www.speakeasy.com/mcp/tool-design/dynamic-tool-discovery)
