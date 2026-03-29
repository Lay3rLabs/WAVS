# Pitfalls Research

**Domain:** Adding WIT-to-schema, MCP execution interface, and OCI distribution to an existing WASM execution platform (WAVS)
**Researched:** 2026-03-24
**Confidence:** HIGH (codebase-verified) / MEDIUM (external sources verified) / LOW (single-source or speculative)

---

## Critical Pitfalls

### Pitfall 1: Trust Tier Confusion — Agent Picks Wrong Tier and Gets Unexpected Behavior

**What goes wrong:**
The agent calls an execution tool without specifying a trust tier, or calls the wrong tier for the use case. The three tiers (result-only / result + signature / on-chain submission) have very different latency profiles, cost characteristics, and guarantees. An agent that defaults to tier 3 for a simple query will trigger on-chain submission and gas costs. An agent that defaults to tier 1 for a high-stakes action gets no cryptographic guarantee. Without explicit tier selection surfaced in the tool schema, the LLM will hallucinate a reasonable-sounding tier based on the tool description.

**Why it happens:**
LLMs fill gaps in underspecified schemas by pattern-matching to plausible defaults. If the tier is not a required parameter with a constrained enum and clear description, the agent will infer a tier from context — and get it wrong. A description like "execute component with verification" is ambiguous between tier 2 and tier 3. The agent cannot know that tier 3 triggers actual blockchain transactions unless the schema says so explicitly.

**How to avoid:**
Make the trust tier a required enum parameter on every execution tool, not optional with a default. Use unambiguous names: `result_only`, `signed_result`, `on_chain`. Add a `destructive: true` annotation or prominent warning in the description of `on_chain`. Consider adding a `dry_run` parameter that simulates tier 3 without submitting. Test by asking an LLM to choose a tier for 10 representative use cases and confirm it selects the expected tier.

**Warning signs:**
- Execution tool has an optional `tier` parameter or no tier parameter at all
- Tool description uses the word "verified" without distinguishing tier 2 from tier 3
- No test coverage verifying which tier gets selected from natural language prompts

**Phase to address:** WIT-to-schema phase (schema must encode tier semantics) and MCP execution interface phase (tool registration must enforce tier as required)

**Confidence:** HIGH — based on known LLM tool-calling behavior and the specific WAVS tier design in PROJECT.md

---

### Pitfall 2: WIT Variant Types Generate Ambiguous or Broken JSON Schemas

**What goes wrong:**
WIT `variant` types (discriminated unions) do not have a canonical JSON Schema representation. A WIT `variant` with cases like `evm-contract-event(trigger-data-evm-contract-event)` can be represented as `oneOf`, `anyOf`, or a tagged union — but LLMs and validators interpret these differently. The `trigger-data` variant in `events.wit` has 7 cases (evm-contract-event, cosmos-contract-event, block-interval, cron, atproto-event, hypercore-append, raw), each with different associated types. Auto-generated schemas from this will either be too permissive (anyOf with no discriminator) or produce schemas that LLMs cannot reliably fill.

The `u128` type in `core.wit` (represented as `tuple<u64, u64>`) is a direct serde compatibility problem: `serde_json` does not support u128 natively, and any JSON Schema generated from it will either represent it as a two-element integer array (confusing) or fail silently.

**Why it happens:**
The WIT type system is richer than JSON Schema in some dimensions (variants with named cases, tuples with semantic positions) and the mapping is not standardized. `component2json`/Wassette's approach embeds WIT documentation as a custom section in the WASM binary, but extracting it and converting to JSON Schema that MCP clients can reliably use is not a solved problem. Recursive types in WIT are also illegal — but `trigger-data` containing `raw(list<u8>)` means any attempt to build a generic trigger-data schema will require special-casing.

**How to avoid:**
Do not attempt to auto-generate schema for the full `trigger-data` variant and surface it directly to agents. Instead, design the MCP execution tool to accept a typed request specific to the use case (e.g., `input_bytes: string` for the `raw` case). The WIT interface describes the component's internal structure; the MCP tool schema describes what the agent provides — these should be decoupled. For the WIT-to-schema tool specifically, produce schemas for individual export functions, not the full world, and add explicit handling for WIT-to-JSON edge cases: `u128` as string, `option<T>` as nullable, `variant` as oneOf with a required `tag` discriminator field.

**Warning signs:**
- Schema for a variant type uses `anyOf` without a required discriminator property
- Any field in generated schema has type `array` with `minItems: 2, maxItems: 2` without explanation (likely a tuple)
- The word "bytes" or `list<u8>` appears in schema as `array of integers` — LLMs will produce wrong values

**Phase to address:** WIT-to-schema phase — must be resolved before MCP execution interface, which depends on usable schemas

**Confidence:** HIGH — based on review of `events.wit`, `core.wit`, and known serde/JSON Schema limitations with WIT types

---

### Pitfall 3: MCP Execution Blocks the Stdio Transport on Long-Running Components

**What goes wrong:**
The existing `wavs-mcp` uses stdio transport. MCP stdio transport is synchronous from the client's perspective: the client sends a request and waits for a response. If a WASM component runs for 30+ seconds (which is within the engine's time limit), the MCP call blocks the stdio channel. Most MCP clients (Claude Code, Cursor) have a 30–60 second timeout. Components that call external APIs or perform blockchain reads can easily hit this. The engine already has dual timeout protection (Wasmtime epoch interrupts + Tokio timeout), but that timeout may be longer than the MCP client's timeout.

This is not theoretical: the community has documented this as the primary operational failure mode for MCP servers serving long-running operations (see MCP issue #1391 and the November 2025 Tasks primitive addition).

**Why it happens:**
The engine's `time_limit_seconds` per workflow defaults to the node config value and can be up to minutes. The MCP execution path needs to respect MCP client timeouts, which are set by the client, not by WAVS. The existing management tools (simulate trigger, exec component) are also blocking, but management operations are infrequent and expected to be fast; execution tools will be called in hot loops by agents.

**How to avoid:**
Set a hard MCP execution timeout of 25 seconds (below the common 30-second MCP client default) that supersedes the workflow's `time_limit_seconds` during MCP-initiated execution. Surface this as a configurable `--mcp-exec-timeout-secs` flag on the `wavs-mcp` binary. For the future: evaluate whether the Tasks primitive (added to MCP 2025-11-25) is appropriate for long-running WAVS workflows, but do not depend on it for the initial implementation — many clients do not support it yet.

**Warning signs:**
- No explicit MCP-layer timeout distinct from the engine's `time_limit_seconds`
- MCP execution tool description does not mention timeout behavior
- E2E test only tests components that return immediately (echo, KV)

**Phase to address:** MCP execution interface phase

**Confidence:** HIGH — based on MCP issue tracker evidence and the existing engine timeout architecture in `execute.rs`

---

### Pitfall 4: Breaking the Existing wavs-mcp Management Interface When Adding Execution Tools

**What goes wrong:**
The existing `wavs-mcp` has 15+ management tools covering deploy, upload, register, simulate, scaffold, and chain-write operations. Adding execution tools to the same server risks: (1) tool name collisions if execution tools use similar names (`wavs_execute_component` vs. `wavs_simulate_trigger`); (2) breaking the `list_tools` response by doubling the tool count, which confuses LLMs with long tool lists; (3) introducing new required startup parameters (`--mcp-exec-*`) that break existing users' config files; (4) accidentally routing execution requests through management code paths.

Research confirms tool name collisions in MCP are a documented production problem: 775 tools across deployed MCP servers have name collisions, and clients like Cursor prefix with `mcp_<server>_<tool_name>` to work around this.

**Why it happens:**
The temptation is to add execution tools as additional entries in the existing `call_tool` match arm and `list_tools` handler. This is the path of least resistance but creates coupling. Management tools require `--token` for write operations; execution tools require a different trust-tier credential model. Mixing these in one handler creates subtle auth path confusion.

**How to avoid:**
Use a clear naming convention: all execution tools share a prefix different from management tools. Current management tools use `wavs_` as a prefix; execution tools should use `wavs_exec_` or `wavs_run_`. Add a `--exec-enabled` flag that must be explicitly set to expose execution tools — this ensures existing users are not surprised. Verify that adding execution tools does not change the behavior or parameters of any existing tool. Write a regression test that runs `list_tools` before and after the addition and diffs the management tool definitions.

**Warning signs:**
- New tool named `wavs_execute_component` when existing tool is named `wavs_simulate_trigger` (confusingly similar)
- New parameters added to existing `WavsMcpServer::new()` signature without default values
- No test that verifies existing tool parameter schemas are unchanged after the addition

**Phase to address:** MCP execution interface phase — treat compatibility as a first-class requirement, not an afterthought

**Confidence:** HIGH — based on direct codebase inspection of `server.rs` and documented MCP tool collision problems

---

### Pitfall 5: OCI Pull Without Digest Verification Enables Supply Chain Attacks

**What goes wrong:**
A `service.json` references an OCI component as `oci://ghcr.io/acme/my-component:latest`. WAVS fetches and caches it at deploy time. If the pull does not verify the layer digest against a pinned value in `service.json`, a registry compromise or mutable tag (`latest`) can silently replace the component being executed. An operator deploys service A, the registry gets compromised, another operator pulls the same tag and gets a malicious component — both services now share the same `service_id` but run different code.

The WASM OCI spec requires `layerDigests` in the config blob, but verifying them is the caller's responsibility. The OCI spec itself does not prevent pulling a tampered layer if the manifest digest matches but the layer content was replaced.

**Why it happens:**
Convenience: mutable tags like `latest` or `v1` are easy to use. Developers copy service.json examples using `latest` tags. The digest is long and ugly. Without a mandatory `oci_digest` field in `service.json` alongside the URI, operators will not pin it.

**How to avoid:**
The `service.json` schema must include a required `digest` field alongside any OCI URI. WAVS must refuse to deploy a service referencing an OCI component without a digest. After pulling, verify the SHA256 of the pulled WASM bytes against the declared digest before loading into the engine. The `ComponentSource::Digest` variant already exists in the types — use it as the canonical source of truth after pull. Cache by digest, not by tag. Emit a warning (and optionally an error) if the OCI URI uses a mutable tag without a digest.

**Warning signs:**
- `service.json` OCI examples use `latest` or version tags without a digest
- No post-pull digest verification step in the pull code path
- The OCI cache key is the URI string, not the digest
- Deploy succeeds with a mismatched digest

**Phase to address:** OCI distribution phase — verification must be in the initial implementation, not added later

**Confidence:** HIGH — based on CNCF WASM OCI spec review and documented 2025 supply chain attacks on mutable tags

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Reuse `wavs_simulate_trigger` as the execution tool with a `trust_tier` parameter | No new tool surface, fast to build | Conflates simulation (dev) with production execution; agents treat them the same and miss the distinction | Never — simulation and execution have fundamentally different semantics |
| Generate WIT-to-JSON Schema from WIT text files at runtime (parse on every call) | No pre-build step required | Parsing WIT at runtime is slow and fragile; WIT toolchain not stable enough for embedding in a hot path | Only during development/debugging |
| Store OCI-pulled WASM blobs in a temp directory without an explicit cache | Simpler code, no cache management | Re-pulls on every deploy; no offline operation; no digest-indexed cache | Never in production |
| Skip the `--exec-enabled` guard and always expose execution tools | Simpler UX (no flag needed) | Existing management-only users see execution tools they cannot use (no deployed services); creates confusion | Never — the guard is cheap and prevents confusion |
| Accept `list<u8>` as a raw byte array in the MCP execution tool schema | Matches the WIT type directly | LLMs cannot reliably produce byte arrays; always wrap bytes as base64 strings | Never in MCP-facing schemas |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| WIT extraction from compiled WASM | Using the text `.wit` files in `wit-definitions/` to generate schema for user components | User components have their own WIT that is embedded in the WASM binary as a custom section; use `wasm-tools component wit` to extract it from the binary, not from the platform WIT files |
| MCP execution ↔ WAVS dispatcher | Routing execution through the full trigger pipeline (TriggerManager → Dispatcher → Engine → Aggregator) for tier 1/2 | Tier 1 and tier 2 only need Engine execution, not Aggregator. Bypass Aggregator for non-on-chain tiers; the `Submit::None` short-circuit already exists for this purpose |
| Trust tier 3 ↔ WAVS node auth | Calling tier 3 without a configured signing credential | Tier 3 requires the node's signing mnemonic for operator signature. The MCP server already handles `signing_mnemonic` for management tools; execution tools must use the same credential path, not a new one |
| OCI pull ↔ `ComponentSource` in service.json | Storing the OCI URI as the `source` field in the deployed service config | After pull and digest verification, convert to `ComponentSource::Digest` before storing; the URI is only for initial resolution, not for runtime identity |
| WIT schema ↔ MCP tool `inputSchema` | Embedding the full `trigger-action` WIT record as the input schema | The MCP input schema should describe what the _agent_ provides (human-readable inputs), not the internal WASM calling convention; translate between them in the tool handler |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Instantiating a new Wasmtime engine per MCP execution call | Execution latency 2-5x higher than the existing CLI exec path | Reuse the engine instance across calls; the existing `ExecComponent::run` creates a new `WTEngine` per call — this is acceptable for CLI but not for a hot MCP path | At >5 concurrent execution calls |
| Pulling OCI component on every deploy request (no cache) | Deploy time grows proportionally to component size and registry latency | Cache by SHA256 digest; check cache before pull; the existing `ComponentDigest` type is the cache key | From the first slow registry response |
| Loading WIT from WASM binary on every `list_tools` call | `list_tools` latency measured in seconds instead of milliseconds | Cache the extracted WIT and generated schema per component digest; invalidate when the service is redeployed | Immediately if the WIT extraction is done on `list_tools` |
| Generating JSON Schema from WIT at schema-tool call time (not pre-baked) | Schema tool returns after 2+ seconds | Pre-generate at service registration time, cache in the node's KV store or a sidecar file | At >10 registered services |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Exposing execution tools without rate limiting | Agent in a loop calls the execution tool thousands of times, exhausting node resources or triggering unintended on-chain transactions | Add per-tool call rate limiting to the MCP server; tier 3 calls should require explicit confirmation or have a much lower rate limit than tier 1/2 |
| Passing agent-provided strings directly as `config` values to the WASM component | Prompt injection: a malicious document processed by a component could include config-key-looking strings that override component behavior | Validate that `config` keys passed through MCP execution tools match the component's declared config schema; never allow agent-provided free-form config in tier 2/3 |
| Not verifying OCI layer digest before execution | Compromised registry pushes malicious WASM that runs with the full permissions of the configured workflow | Always verify SHA256 of pulled bytes against declared digest; fail closed (refuse execution) on mismatch |
| Tool name collision between execution tools and management tools | Agent calls `wavs_exec_my_service` when it meant `wavs_simulate_trigger_my_service`, triggering real execution | Strict naming prefix convention; execution tools use `wavs_exec_` prefix; simulation tools retain `wavs_simulate_` prefix; add tool descriptions that contrast the two |
| Granting `AllowedHostPermission::All` to components executed via MCP | Components execute as a trusted tool for the agent but can make arbitrary outbound requests (SSRF, data exfiltration) | The existing `AllowedHostPermission::Only(allowlist)` should be the default for MCP-executed components unless the service explicitly requests `All`; surface the permission level in the tool description |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Schema for a component's inputs requires the full `trigger-action` struct | Agent cannot figure out what to pass; asks the user for clarification or hallucinates a structure | Surface only the relevant inner data as the schema (e.g., for a price-feed component, the schema is `{"symbol": "string"}` not the full trigger wrapper) |
| Trust tier explanation buried in tool description prose | Agent picks a tier based on name alone, not semantics | Put tier semantics in a `// Tier X: ...` comment in the schema description field, not just the tool description |
| OCI pull errors surface as generic "component not found" | Developer cannot tell whether the pull failed, digest mismatched, or the registry is unavailable | Return structured errors distinguishing: pull_failed / digest_mismatch / registry_unavailable / already_cached |
| WIT-to-schema tool returns a schema the agent cannot use (raw WIT types like `option<list<u8>>`) | Agent produces invalid inputs, component execution fails | The schema tool must return LLM-ready JSON Schema, not a direct WIT-to-JSON mechanical translation; add a `for_llm: bool` parameter that enables friendly type mappings |

---

## "Looks Done But Isn't" Checklist

- [ ] **WIT-to-schema:** Schema is generated for the component's exported functions — verify it also handles imported WIT types transitively (a component may use `wavs:types/core` types, which must be resolved in the schema)
- [ ] **Trust tier 3:** On-chain submission pathway calls the Aggregator and gets multi-operator quorum — verify that a single-operator test node actually produces a valid on-chain result before declaring tier 3 done
- [ ] **OCI pull:** Pull succeeds from `ghcr.io` with a real component — verify that the pulled bytes, when loaded into Wasmtime, match the digest in `service.json` AND that the engine executes them identically to a locally uploaded component
- [ ] **MCP execution tools registered:** `list_tools` returns the new tools — verify that calling an execution tool with invalid input returns a structured MCP error, not a panic or unstructured stderr message
- [ ] **Existing management tools unchanged:** Run the full existing MCP tool test suite after adding execution tools — verify no parameter schemas changed and no existing tool errors

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Trust tier confusion in production | HIGH | Requires re-educating all deployed agents (update system prompts), cannot retroactively fix incorrect on-chain submissions; tier 3 calls cannot be undone |
| WIT variant schema breaks LLM tool calling | MEDIUM | Update schema generation, re-register affected services; agents using cached tool descriptions will need to re-fetch |
| OCI supply chain compromise via mutable tag | HIGH | Revoke compromised service, re-deploy from known-good digest, audit all services that used the compromised component; on-chain submissions from the bad component cannot be reverted |
| MCP stdio timeout blocking | LOW | Reduce `--mcp-exec-timeout-secs` to a value below client timeout; does not require redeploy of services |
| Breaking existing management tool schemas | MEDIUM | Revert the breaking change in wavs-mcp, re-publish binary; clients using the old schema continue to work until they upgrade |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Trust tier confusion | WIT-to-schema (schema encodes tier semantics) + MCP execution interface (tier as required enum) | Ask an LLM to select a tier for 5 representative agent tasks; all must select correctly |
| WIT variant/u128 schema edge cases | WIT-to-schema phase | Schema for `trigger-data` passes JSON Schema validation; generated schema for a component with a variant type can be filled by an LLM without errors |
| MCP stdio blocking on long-running execution | MCP execution interface phase | Run a component that sleeps for 31 seconds; MCP client receives a timeout error, not a connection hang |
| Breaking existing management interface | MCP execution interface phase | `list_tools` diff test passes; all existing management tool tests pass unchanged |
| OCI pull without digest verification | OCI distribution phase | Deploy with a digest, tamper with the cached bytes, verify deploy fails with `digest_mismatch` error |
| AllowedHostPermission::All default for MCP-exec | MCP execution interface phase | Verify default permissions for MCP-initiated execution are restrictive; `All` requires explicit opt-in |

---

## Sources

- WAVS codebase: `packages/wavs-mcp/src/server.rs`, `packages/engine/src/worlds/operator/execute.rs`, `wit-definitions/types/wit/events.wit`, `wit-definitions/types/wit/core.wit`
- [OWASP MCP Top 10](https://owasp.org/www-project-mcp-top-10/)
- [MCP Security Best Practices](https://modelcontextprotocol.io/specification/draft/basic/security_best_practices)
- [WebAssembly Component Model: Support Recursive Values (Issue #430)](https://github.com/WebAssembly/component-model/issues/430) — resolved March 2025
- [CNCF WASM OCI Artifact Specification](https://tag-runtime.cncf.io/wgs/wasm/deliverables/wasm-oci-artifact/)
- [Distributing WebAssembly Components using OCI Registries](https://opensource.microsoft.com/blog/2024/09/25/distributing-webassembly-components-using-oci-registries/)
- [Tool-space interference in MCP era (Microsoft Research)](https://www.microsoft.com/en-us/research/blog/tool-space-interference-in-the-mcp-era-designing-for-agent-compatibility-at-scale/)
- [MCP tool name collisions — Cursor community bug report](https://forum.cursor.com/t/mcp-tools-name-collision-causing-cross-service-tool-call-failures/70946)
- [MCP Long-Running Operations issue #1391](https://github.com/modelcontextprotocol/modelcontextprotocol/issues/1686)
- [Why Your MCP Agent Keeps Timing Out](https://medium.com/@ai_transfer_lab/why-your-mcp-agent-keeps-timing-out-and-the-fix-that-just-shipped-ad9cb130f8c4)
- [Wassette: Microsoft security-oriented WASM MCP runtime](https://github.com/microsoft/wassette)
- [serde-json u128 compatibility issue](https://github.com/serde-rs/json/issues/502)
- [Tool Shadowing/Name Collisions](https://modelcontextprotocol-security.io/ttps/tool-poisoning/tool-shadowing/)

---
*Pitfalls research for: WAVS improvements — WIT-to-schema, MCP execution interface, OCI distribution*
*Researched: 2026-03-24*
