# Phase 3: MCP Execution Interface - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-25
**Phase:** 03-mcp-execution-interface
**Areas discussed:** Tool naming & discovery, Trust tier response contract, On-chain gating & safety, Error & timeout responses

---

## Tool Naming & Discovery

### Q1: Naming prefix

| Option | Description | Selected |
|--------|-------------|----------|
| wavs_exec_ (Recommended) | Clearer separation from management tools. 'exec' signals code execution. Matches EXEC-07. | ✓ |
| wavs_run_ | Shorter, more natural. Matches ROADMAP wording. | |
| wavs_call_ | RPC-style naming. Familiar to web3 developers. | |

**User's choice:** wavs_exec_
**Notes:** Resolves ROADMAP vs REQUIREMENTS conflict in favor of REQUIREMENTS (EXEC-07).

### Q2: Tool name derivation

| Option | Description | Selected |
|--------|-------------|----------|
| One tool per workflow (Recommended) | wavs_exec_{service}_{workflow}. Matches success criteria. | ✓ |
| One tool per exported function | wavs_exec_{service}_{workflow}_{function}. More granular. | |
| One tool per service | wavs_exec_{service}. Workflow as parameter. | |

**User's choice:** One tool per workflow (default)
**Notes:** "In v2 we can get smarter about surfacing components that access different functions than our standalone run interface."

### Q3: Tool description content

| Option | Description | Selected |
|--------|-------------|----------|
| Rich description (Recommended) | Service name, workflow purpose, supported trust tiers, component source. | ✓ |
| Minimal description | Just workflow name and trust tier info. | |
| You decide | Claude's discretion. | |

**User's choice:** Rich description
**Notes:** None

### Q4: Tool list caching

| Option | Description | Selected |
|--------|-------------|----------|
| Single unified cache (Recommended) | One cache for full tool list. Invalidated by service events. | |
| Separate cache for exec tools | Exec tools cached with 5s TTL. Management tools static. | |
| You decide | Claude's discretion. | |

**User's choice:** "Whichever is most performant"
**Notes:** Deferred to Claude's discretion with performance as the guiding criterion.

---

## Trust Tier Response Contract

### Q1: Tier 1 response format

| Option | Description | Selected |
|--------|-------------|----------|
| Structured envelope (Recommended) | Always return {trust_tier, result, execution_time_ms}. Consistent across tiers. | |
| Raw result | Return component output directly. Simpler. | ✓ |
| You decide | Claude's discretion. | |

**User's choice:** Raw result
**Notes:** Keep Tier 1 simple — no wrapper envelope.

### Q2: Tier 2 cryptographic encoding

| Option | Description | Selected |
|--------|-------------|----------|
| Hex-encoded (Recommended) | 0x-prefixed hex strings. Standard in EVM/web3. Matches alloy types. | |
| Base64-encoded | More compact. Common in non-EVM crypto. | |
| You decide | Claude's discretion. | ✓ |

**User's choice:** You decide
**Notes:** Deferred to Claude. Hex is natural fit given alloy/EVM ecosystem.

### Q3: Tier 3 response content

| Option | Description | Selected |
|--------|-------------|----------|
| Hash + chain info (Recommended) | {tx_hash, chain_id, block_explorer_url}. Actionable info. | ✓ (partial) |
| Hash only | Just transaction hash. Minimal. | |
| Full tx receipt | Wait for confirmation. Full receipt with status, gas, block. | ✓ (partial) |

**User's choice:** "Option for either 1 or 3"
**Notes:** Default to hash + chain info, with optional `wait_for_receipt: true` parameter for full receipt.

### Q4: Trust tier availability per tool

| Option | Description | Selected |
|--------|-------------|----------|
| Always accept all three (Recommended) | Every exec tool accepts all tiers. Error if disabled. | ✓ |
| Advertise available tiers per tool | Schema shows only enabled tiers. Changes with config. | |

**User's choice:** Always accept all three
**Notes:** None

---

## On-Chain Gating & Safety

### Q1: Disabled Tier 3 behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Error with clear message (Recommended) | Structured error explaining tier is not enabled. | ✓ |
| Fallback to signed_result | Silently downgrade to Tier 2. | |
| Fallback with explicit warning | Downgrade with prominent warning field. | |

**User's choice:** Error with clear message
**Notes:** None

### Q2: Gating granularity

| Option | Description | Selected |
|--------|-------------|----------|
| Two-level gating is sufficient (Recommended) | Global --exec-enabled + per-service exec_enabled in service.json. | ✓ |
| Add allowlist/denylist | Additional --exec-allow/--exec-deny flags per service. | |
| You decide | Claude's discretion. | |

**User's choice:** Two-level gating is sufficient
**Notes:** None

### Q3: Confirmation mechanism for Tier 3

| Option | Description | Selected |
|--------|-------------|----------|
| No confirmation (Recommended) | Agent chose on_chain — that's the confirmation. | |
| Optional dry-run parameter | simulate: true runs without submitting. | |
| Cost estimate first | Return gas estimate, require follow-up to confirm. | ✓ |

**User's choice:** Cost estimate first
**Notes:** Two-step flow for Tier 3: estimate → confirm. Protects agents managing funds.

---

## Error & Timeout Responses

### Q1: Timeout error format

| Option | Description | Selected |
|--------|-------------|----------|
| Structured MCP error (Recommended) | isError: true with {code, message, elapsed_ms, component}. Programmatically detectable. | ✓ |
| Simple text error | isError: true with text description. | |
| You decide | Claude's discretion. | |

**User's choice:** Structured MCP error
**Notes:** None

### Q2: Error code scope

| Option | Description | Selected |
|--------|-------------|----------|
| Structured codes for all errors (Recommended) | EXECUTION_TIMEOUT, TIER_NOT_ENABLED, SERVICE_NOT_FOUND, COMPONENT_FAILED, SIGNING_FAILED, SUBMISSION_FAILED. | ✓ |
| Structured for critical, text for rest | Error codes only for timeout and tier-gating. | |
| You decide | Claude's discretion. | |

**User's choice:** Structured codes for all errors
**Notes:** None

### Q3: Partial result handling

| Option | Description | Selected |
|--------|-------------|----------|
| Return partial result (Recommended) | Include component output in error response if execution succeeded but signing/submission failed. | ✓ |
| Error only, no partial result | Clean error. Agent retries full call. | |
| You decide | Claude's discretion. | |

**User's choice:** Return partial result
**Notes:** Avoids wasting successful execution compute.

### Q4: Timeout configurability

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed 25s (Recommended) | EXEC-08 enforced. Simple and predictable. | |
| Configurable with 25s default | Optional timeout_ms, capped at 25s. Agents can request shorter. | ✓ |
| You decide | Claude's discretion. | |

**User's choice:** Configurable with 25s default
**Notes:** Allows fast-fail scenarios while maintaining 25s cap per EXEC-08.

---

## Claude's Discretion

- Tool list caching strategy (performance-optimized)
- Cryptographic data encoding for Tier 2 (hex likely given EVM ecosystem)
- Whether `wait_for_receipt` ships in v1 or deferred
- Internal execution pathway (aggregator bypass for Tier 1)
- `notifications/tools/list_changed` wiring details
- Gas estimation implementation for Tier 3 two-step flow

## Deferred Ideas

- Per-function tool granularity — V2 feature for smarter component function surfacing
- Per-service allowlist/denylist — not needed with current two-level gating
