# Execution Flow

Execute a deployed WAVS service workflow directly through MCP tools.

---

## Prerequisites

1. **Service is deployed** — the service must appear in `wavs_list_services` output
2. **MCP server started with `--exec-enabled`** — without this flag, `wavs_exec_*` tools do not appear
3. **Tier-specific config** (see [Trust Tier Selection](#trust-tier-selection) below)

---

## Checklist

- [ ] **Step 1** — `wavs:wavs_list_services` — Find the deployed service and note its name + workflow IDs.
- [ ] **Step 2** — Identify the execution tool: `wavs_exec_{service_name}_{workflow_id}` (service name is lowercased, non-alphanumeric chars become `_`, max 64 chars).
- [ ] **Step 3** — Choose a trust tier based on your needs (see table below).
- [ ] **Step 4** — Call the execution tool with `trust_tier`, `input`, and optional `timeout_ms`.
- [ ] **Step 5 (Tier 3 only)** — Receive gas estimate + nonce, then call again with `confirm: "<nonce>"` within 60 seconds.

---

## Trust Tier Selection

| Tier | `trust_tier` value | What you get | MCP server requirements | When to use |
|------|-------------------|--------------|------------------------|-------------|
| **1** | `result_only` | Raw component output (text or hex) | `--exec-enabled` | Quick testing, data queries, no trust guarantees needed |
| **2** | `signed_result` | Component output + operator signature | `--exec-enabled` + `--signing-mnemonic` | Verifiable off-chain results, attestations |
| **3** | `on_chain` | Transaction hash (result submitted to chain) | `--exec-enabled` + `--signing-mnemonic` + `--mcp-chain-credential` + service `exec_enabled: true` | On-chain settlement, triggering contract state changes |

---

## Examples

### Tier 1 — result_only

```
wavs_exec_echo_data_default(
  trust_tier="result_only",
  input={"message": "Hello WAVS"}
)
→ Hello WAVS
```

The raw component output is returned. If the payload is valid UTF-8, it is shown as text; otherwise as `0x`-prefixed hex.

### Tier 2 — signed_result

```
wavs_exec_echo_data_default(
  trust_tier="signed_result",
  input={"message": "Hello WAVS"}
)
→ {
    "payload": "0x48656c6c6f2057415653",
    "signature": "0xabc123...",
    "signer": "0xdef456..."
  }
```

The component output is wrapped with the operator's cryptographic signature. The signer address corresponds to the service's HD-derived signing key (viewable via `wavs_get_service_signer`).

**Requires:** `--signing-mnemonic` configured on the MCP server (same mnemonic the WAVS node uses).

### Tier 3 — on_chain (two-step)

**Step 1: Estimate gas**
```
wavs_exec_my_service_default(
  trust_tier="on_chain",
  input={"data": "some payload"}
)
→ {
    "status": "pending_confirmation",
    "nonce": "0018a3f5b2c1d4e6",
    "gas_estimate": "210000",
    "chain": "evm:31337",
    "message": "Confirm within 60 seconds by passing confirm=\"0018a3f5b2c1d4e6\""
  }
```

**Step 2: Confirm submission** (must call within 60 seconds)
```
wavs_exec_my_service_default(
  trust_tier="on_chain",
  confirm="0018a3f5b2c1d4e6"
)
→ {
    "status": "submitted",
    "tx_hash": "0x789abc..."
  }
```

**Requires:**
- `--signing-mnemonic` and `--mcp-chain-credential` on the MCP server
- `exec_enabled: true` in the service definition (see [`reference/service-json.md`](../reference/service-json.md))
- The nonce expires after 60 seconds — if missed, re-execute from Step 1

---

## Error Codes

| Error Code | Meaning | Common Cause |
|------------|---------|-------------|
| `EXECUTION_TIMEOUT` | Component did not complete within `timeout_ms` | Increase `timeout_ms` (max 25000), or the component is hanging |
| `TIER_NOT_ENABLED` | Requested tier is not available | Missing `--signing-mnemonic` (Tier 2), `--mcp-chain-credential` (Tier 3), or `exec_enabled: true` (Tier 3) |
| `SERVICE_NOT_FOUND` | Tool name does not match any deployed service | Service may have been deleted; call `wavs_list_services` to check |
| `COMPONENT_FAILED` | WASM component returned an error or no output | Check component logic; use `wavs_query_component_logs` to see `host::log()` output |
| `SIGNING_FAILED` | Operator signature could not be produced (Tier 2) | Verify `--signing-mnemonic` matches the WAVS node's mnemonic |
| `SUBMISSION_FAILED` | On-chain transaction reverted or failed (Tier 3) | Check gas, chain RPC health, contract state; the error may include a `partial_result` with the raw component output |

Errors include a `partial_result` field when the component succeeded but a later step (signing/submission) failed. The partial result contains the raw hex-encoded component output so it is not lost.

---

## Debugging

### Check component logs

Use `wavs_query_component_logs` to see what the component printed via `host::log()`:

```
wavs_query_component_logs(
  service_id="<64-char hex from wavs_list_services>",
  level="debug"
)
→ { "entries": [...], "next_id": 42 }
```

Filter further with `workflow_id` or `digest` parameters.

### Check node-level logs

Use `wavs_query_logs` for broader system logs:

```
wavs_query_logs(
  target="wavs::subsystems::engine",
  level="warn"
)
```

### Common issues

| Symptom | Investigation |
|---------|--------------|
| Tool not in `list_tools` | Verify `--exec-enabled` and that the service is deployed |
| `SERVICE_NOT_FOUND` after deploy | Service cache has a 5-second TTL — wait and retry, or call `wavs_list_services` first |
| `TIER_NOT_ENABLED` for Tier 3 | Check both `--mcp-chain-credential` and `exec_enabled: true` in service definition |
| Confirmation expired | Nonces expire after 60 seconds — re-execute the tool to get a fresh estimate |
| Garbled output | Component may be returning binary data — check if it should produce hex or UTF-8 |
