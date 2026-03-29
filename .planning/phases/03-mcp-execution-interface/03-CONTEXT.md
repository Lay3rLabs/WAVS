# Phase 3: MCP Execution Interface - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

AI agents discover and invoke deployed WAVS service components as callable MCP tools via the existing `wavs-mcp` server. Each deployed service workflow appears as a tool with auto-generated `inputSchema` from Phase 2's WIT-to-schema library. Agents choose an explicit trust tier per call: result only, signed result, or on-chain submission. Service deploy/remove fires `notifications/tools/list_changed`. Global `--exec-enabled` flag gates all execution tools. 25s configurable timeout at MCP layer.

</domain>

<decisions>
## Implementation Decisions

### Tool Naming & Discovery
- **D-01:** Execution tools use `wavs_exec_` prefix (not `wavs_run_`). Resolves conflict between ROADMAP and REQUIREMENTS in favor of EXEC-07. Update ROADMAP to match.
- **D-02:** One tool per deployed service workflow: `wavs_exec_{service}_{workflow}`. V2 can get smarter about surfacing components that access different functions than the standalone run interface.
- **D-03:** Rich tool descriptions: include service name, workflow purpose (from WIT doc comments), supported trust tiers, and component source (OCI URI or local path). Helps agents pick the right tool.
- **D-04:** Tool list caching strategy is Claude's discretion — optimize for performance. STATE.md flagged 5s TTL design; choose unified or separate cache based on what performs best.

### Trust Tier Response Contract
- **D-05:** Tier 1 (`result_only`) returns the raw component output directly as MCP tool result content. No wrapper envelope — keep it simple.
- **D-06:** Tier 2 (`signed_result`) wraps the result in a structured envelope with operator signature and signer public key. Cryptographic data encoding is Claude's discretion (hex is natural fit given alloy/EVM ecosystem).
- **D-07:** Tier 3 (`on_chain`) defaults to returning `{tx_hash, chain_id, block_explorer_url}`. Optional `wait_for_receipt: true` parameter returns full transaction receipt instead (status, gas used, block number). Note: waiting for receipt may consume more of the timeout budget.
- **D-08:** All three trust tiers are always accepted as input on every exec tool. If a tier is disabled for a service, return a structured error (don't silently downgrade).
- **D-09:** Tier 3 has a two-step flow: first call returns a gas cost estimate, agent must confirm with a follow-up call to actually submit the transaction. Protects agents managing funds from unexpected costs.

### On-Chain Gating & Safety
- **D-10:** Two-level gating is sufficient: global `--exec-enabled` CLI flag on MCP server + per-service `exec_enabled` in service.json. No additional allowlist/denylist needed.
- **D-11:** When Tier 3 is requested but not enabled for a service, return a structured error: "on_chain tier not enabled for this service". No fallback to lower tiers.
- **D-12:** No interactive confirmation for on-chain submission — the agent explicitly chose the `on_chain` tier, which IS the confirmation. The two-level gating + cost estimate step (D-09) provide sufficient safety.

### Error & Timeout Responses
- **D-13:** All execution errors use structured error codes. Defined codes: `EXECUTION_TIMEOUT`, `TIER_NOT_ENABLED`, `SERVICE_NOT_FOUND`, `COMPONENT_FAILED`, `SIGNING_FAILED`, `SUBMISSION_FAILED`. Agents can programmatically handle each case.
- **D-14:** Timeout is configurable per-call via optional `timeout_ms` parameter, capped at 25s (EXEC-08). Default is 25s.
- **D-15:** If component executes successfully but signing (Tier 2) or submission (Tier 3) fails, the raw component result is included in the error response alongside the error code. Avoids wasting successful execution.

### Claude's Discretion
- Tool list caching implementation (unified vs separate, TTL value)
- Cryptographic data encoding format for Tier 2 signatures (hex recommended given EVM ecosystem)
- Whether to implement `wait_for_receipt` in v1 or defer to v2
- Internal execution pathway: whether Tier 1 bypasses aggregator or goes through existing pipeline
- How `notifications/tools/list_changed` is wired to service deploy/remove events
- Gas estimation implementation details for the Tier 3 two-step flow

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### MCP Server (existing codebase)
- `packages/wavs-mcp/src/server.rs` — Existing MCP server with 26 management tools, `list_tools()` and `call_tool()` dispatch pattern, `schema_for_type::<T>()` for JSON Schema generation
- `packages/wavs-mcp/src/client.rs` — HTTP client wrapper for WAVS node communication
- `packages/wavs-mcp/src/chain_ops.rs` — On-chain transaction signing and submission logic (reuse for Tier 3)
- `packages/wavs-mcp/src/main.rs` — CLI args including credential flags

### WIT-to-Schema (Phase 2 output)
- `packages/wit-schema/src/lib.rs` — `generate_schema()` and `generate_schema_cached()` public API; returns `{world, exports: {fn_name: {inputSchema, outputSchema}}, $defs}`
- `packages/wit-schema/src/cache.rs` — Schema caching by component digest

### Service & Execution Model
- `packages/types/src/service.rs` — `Service`, `Workflow`, `Trigger`, `Component`, `Submit` types
- `packages/wavs/src/dispatcher.rs` — Trigger → Engine → Aggregator → Submission pipeline
- `packages/wavs/src/subsystems/engine.rs` — `EngineCommand::ExecuteOperator` for component execution
- `packages/wavs/src/subsystems/submission.rs` — Per-service HD-derived signing keys, single-operator signing support
- `packages/wavs/src/http/handlers/service/` — Service CRUD HTTP endpoints

### Signing Infrastructure
- `packages/types/src/signing.rs` — `WavsSignable` and `WavsSigner` traits, `WavsSignature`, `SignatureKind`
- `packages/types/src/submission.rs` — Submission envelope structure

### Requirements
- `.planning/REQUIREMENTS.md` §MCP Execution — EXEC-01 through EXEC-08

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `WavsMcpServer` (`wavs-mcp/src/server.rs`): Full MCP server implementation with tool registration, dispatch, and error handling patterns. Phase 3 extends this with execution tools.
- `schema_for_type::<T>()` (`wavs-mcp/src/server.rs`): Converts Rust structs to JSON Schema via schemars — use for exec tool parameter schemas.
- `WavsClient` (`wavs-mcp/src/client.rs`): HTTP client for WAVS node — extend with execution trigger endpoints.
- `generate_schema_cached()` (`wit-schema/src/lib.rs`): Phase 2 schema generation — provides `inputSchema` for each tool.
- `ComponentDigest` (`types/src/id/hash.rs`): SHA256 digest for component identity and caching.
- `WavsSigner` trait (`types/src/signing.rs`): Single-operator signing without full aggregator — key for Tier 2.

### Established Patterns
- Tool definitions in `list_tools()` follow: name, description, `schema_for_type::<ParamStruct>()` for inputSchema
- Tool dispatch in `call_tool()` via match on tool name string
- Credential gating: `require_mcp_chain_credential()` / `require_signing_mnemonic()` guards — reuse pattern for `--exec-enabled`
- Error handling: `McpError` → `ErrorData` with code/message → `CallToolResult::error()`
- All JSON serialization via serde_json

### Integration Points
- `list_tools()` in `server.rs` — add dynamic exec tools alongside static management tools
- `call_tool()` in `server.rs` — add exec tool dispatch
- `main.rs` CLI args — add `--exec-enabled` flag
- Service deploy/remove handlers — wire `notifications/tools/list_changed` notification
- `WavsClient` — add methods to trigger component execution and retrieve results

</code_context>

<specifics>
## Specific Ideas

- V2 vision: smarter tool surfacing that exposes individual component functions rather than just workflows
- Tier 3 two-step flow modeled after cost estimation patterns: estimate first, confirm to submit
- Tool descriptions should be rich enough that agents can pick the right tool without trial-and-error

</specifics>

<deferred>
## Deferred Ideas

- Per-function tool granularity (V2 — smarter component function surfacing beyond one-tool-per-workflow)
- Per-service allowlist/denylist for exec tools (not needed with two-level gating)

</deferred>

---

*Phase: 03-mcp-execution-interface*
*Context gathered: 2026-03-25*
