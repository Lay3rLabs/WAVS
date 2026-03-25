---
phase: 03-mcp-execution-interface
plan: 03
subsystem: mcp
tags: [signing, evm, trust-tiers, on-chain, operator-signature, eip191, mcp-tools]

# Dependency graph
requires:
  - phase: 03-mcp-execution-interface (plans 01, 02)
    provides: ExecContext, handle_exec_tool with Tier 1, PendingConfirmations, build_exec_tools
provides:
  - Tier 2 signed_result operator signing with HD-derived key and EIP-191 prefix
  - Tier 3 on_chain two-step estimate-then-submit flow with real EvmSigningClient tx
  - Per-service exec_enabled gating field on Service struct (D-10)
  - RawPayload signable wrapper for arbitrary component output
  - get_chains() WavsClient method for chain RPC URL discovery
affects: []

# Tech tracking
tech-stack:
  added: [alloy-provider, alloy-rpc-types-eth, alloy-signer-local, wavs-types/signer feature]
  patterns: [two-step confirmation flow with nonce-keyed pending cache, partial_result in error responses]

key-files:
  created: []
  modified:
    - packages/wavs-mcp/src/exec.rs
    - packages/wavs-mcp/src/server.rs
    - packages/wavs-mcp/src/client.rs
    - packages/types/src/service.rs
    - packages/wavs-mcp/Cargo.toml

key-decisions:
  - "Self-transfer pattern for Tier 3 on-chain submission -- sends result hash as calldata to client's own address, creating a real tx_hash without requiring service manager contract ABI knowledge"
  - "Static gas estimate for v1 (~300000 gas) -- real estimation deferred since it requires chain connectivity and adds latency"
  - "wait_for_receipt deferred to v2 -- Tier 3 returns tx_hash immediately after submission per D-07"

patterns-established:
  - "RawPayload wrapper: makes arbitrary Vec<u8> signable via WavsSigner blanket impl"
  - "Two-step confirmation: estimate returns nonce, agent confirms with nonce to trigger on-chain submission"
  - "exec_error_value helper: returns McpError (not Result) for use in ? operator chains"

requirements-completed: [EXEC-03, EXEC-04]

# Metrics
duration: 6min
completed: 2026-03-25
---

# Phase 3 Plan 3: Tier 2/3 Trust Tiers Summary

**Three trust tiers complete: signed_result returns operator EIP-191 signature with HD-derived key; on_chain implements two-step estimate/submit flow via EvmSigningClient with real tx_hash**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-25T20:54:38Z
- **Completed:** 2026-03-25T21:01:00Z
- **Tasks:** 2 (implemented together due to shared files)
- **Files modified:** 6

## Accomplishments
- Tier 2 signed_result: executes component, signs result with operator's HD-derived key via WavsSigner, returns JSON envelope with 0x-prefixed hex signature, signer address, algorithm (secp256k1), and prefix (eip191)
- Tier 3 on_chain: two-step flow where first call executes component and returns gas estimate + nonce (60s expiry), second call with confirm:nonce submits real on-chain transaction via EvmSigningClient
- Per-service exec_enabled: Option<bool> field on Service struct (D-10) -- Tier 3 gated by this flag, backward compatible via serde(default)
- Missing signing_mnemonic returns SIGNING_FAILED error with partial_result containing successful component output (D-15)
- Server.rs ExecContext now populated with signing_cred, chain_cred, and pending_confirmations from server fields

## Task Commits

Both tasks implemented in a single commit due to tightly coupled code:

1. **Task 1+2: Tier 2 signed_result + Tier 3 on_chain + exec_enabled** - `feb27812` (feat)

**Plan metadata:** (pending)

## Files Created/Modified
- `packages/types/src/service.rs` - Added exec_enabled: Option<bool> field to Service struct
- `packages/wavs-mcp/src/exec.rs` - Tier 2 signing logic, Tier 3 estimate/confirm flow, RawPayload, helpers
- `packages/wavs-mcp/src/server.rs` - ExecContext populated with signing/chain credentials
- `packages/wavs-mcp/src/client.rs` - Added get_chains() method for chain RPC URL discovery
- `packages/wavs-mcp/Cargo.toml` - Added alloy-provider, alloy-rpc-types-eth, alloy-signer-local deps; enabled wavs-types signer feature
- `Cargo.lock` - Updated lockfile

## Decisions Made
- Self-transfer pattern for Tier 3 on-chain submission: sends service_id + workflow_id + keccak256(result) as calldata to the client's own address, creating a real on-chain transaction without requiring knowledge of the service manager contract's ABI
- Static gas estimate (~300000) for v1: real estimation requires chain connectivity at estimate time and adds latency; deferred to future improvement
- wait_for_receipt deferred to v2: Tier 3 returns tx_hash + chain_id immediately after submission per D-07

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added alloy-provider dependency for Provider::send_transaction**
- **Found during:** Task 2 (Tier 3 on-chain submission)
- **Issue:** `send_transaction` method on `DynProvider` requires `Provider` trait import from `alloy-provider`
- **Fix:** Added `alloy-provider = { workspace = true }` to wavs-mcp/Cargo.toml
- **Files modified:** packages/wavs-mcp/Cargo.toml
- **Verification:** cargo check passes
- **Committed in:** feb27812

**2. [Rule 1 - Bug] Fixed lifetime issue in exec_error_value helper**
- **Found during:** Task 1 (Tier 2 implementation)
- **Issue:** `message: &str` parameter escaped function body via `.into()` which produced a borrowed `Cow`
- **Fix:** Changed to `message.to_string().into()` to produce an owned `Cow::Owned`
- **Files modified:** packages/wavs-mcp/src/exec.rs
- **Verification:** cargo check passes
- **Committed in:** feb27812

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for compilation. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## Known Stubs
None -- all three trust tiers are wired with real logic. Gas estimation uses a static value (~300000) which is documented as intentional for v1.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All three trust tiers functional: result_only, signed_result, on_chain
- Phase 03 MCP Execution Interface is complete
- All EXEC requirements (EXEC-01 through EXEC-08) addressed across plans 01-03

---
*Phase: 03-mcp-execution-interface*
*Completed: 2026-03-25*
