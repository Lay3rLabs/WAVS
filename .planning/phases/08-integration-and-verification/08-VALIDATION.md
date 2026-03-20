---
phase: 8
slug: integration-and-verification
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust integration tests (cargo test) |
| **Config file** | `packages/layer-tests/layer-tests.toml` |
| **Quick run command** | `cargo test -p layer-tests -- bls` |
| **Full suite command** | `cargo test -p layer-tests` |
| **Estimated runtime** | ~120 seconds (full E2E with anvil) |

---

## Sampling Rate

- **After every task commit:** Run `cargo build -p layer-tests 2>&1 | grep error` (compile check)
- **After every plan wave:** Run `cargo test -p layer-tests -- bls`
- **Before `/gsd:verify-work`:** Full suite must be green (`cargo test -p layer-tests`)
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 8-01-01 | 01 | 1 | INT-01 | compile | `cargo build -p utils --lib && cargo build -p layer-tests --lib` | W0 (creates operator.rs changes, evm.rs changes, SimpleBlsSubmit.sol) | pending |
| 8-01-02 | 01 | 1 | INT-01 | compile | `cargo build -p utils --lib` | W0 (creates middleware_poa_bls.rs, updates common.rs, mod.rs) | pending |
| 8-02-01 | 02 | 2 | INT-01 | compile | `cargo build -p layer-tests --lib` | W0 (updates matrix.rs, test_registry.rs, test_definition.rs, solidity_types.rs) | pending |
| 8-02-02 | 02 | 2 | INT-01, INT-02 | compile | `cargo build -p layer-tests --lib` | W0 (updates service_managers.rs, helpers.rs, handles.rs) | pending |
| 8-02-03 | 02 | 2 | INT-01, INT-02 | integration | `cargo test -p layer-tests -- bls_multi_operator` | Depends on 8-02-01, 8-02-02 | pending |

*Status: pending | green | red | flaky*

---

## Wave 0 Requirements

- [ ] `packages/utils/src/test_utils/middleware/evm/middleware_poa_bls.rs` — PoaBlsMiddleware (local forge-based deployment)
- [ ] `packages/utils/src/test_utils/middleware/evm/common.rs` — `EvmMiddlewareType::PoaBls` variant added
- [ ] `packages/utils/src/test_utils/middleware/operator.rs` — `bls_pubkey`, `bls_proof` fields on AvsOperator
- [ ] `packages/layer-tests/src/e2e/handles/evm.rs` — Prague hardfork flag support
- [ ] `packages/layer-tests/src/e2e/matrix.rs` — `EvmService::BlsMultiOperator` variant
- [ ] `packages/layer-tests/src/e2e/test_registry.rs` — BLS test registration
- [ ] `packages/layer-tests/src/e2e/test_definition.rs` — `bls: bool` field on TestDefinition
- [ ] `packages/layer-tests/src/e2e/service_managers.rs` — BLS operator registration path + per-test middleware dispatch
- [ ] `packages/layer-tests/src/e2e/helpers.rs` — BLS-aware submit config creation
- [ ] `packages/layer-tests/src/e2e/handles.rs` — EvmMiddlewares struct (both Poa and PoaBls created upfront)
- [ ] `examples/contracts/solidity/mocks/SimpleBlsSubmit.sol` — BLS-compatible submit contract
- [ ] `examples/contracts/solidity/interfaces/bls/IWavsServiceHandler.sol` — BLS interface vendored from poa-middleware

*Wave 0 covers new test infrastructure; existing secp256k1 tests already exist.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| BLS pairing check passes on-chain | INT-01 | Requires live EIP-2537 precompile call and on-chain verification | Run `cargo test -p layer-tests` with isolated `bls_multi_operator` and confirm test passes |
| No secp256k1 regressions | INT-02 | Full suite run with human review of output | Run `cargo test -p layer-tests` with isolated `echo_data` and confirm test passes |
| Mixed-mode BLS + secp256k1 coexistence | INT-02 | Per-test middleware dispatch must work in combined matrix | Run with both `echo_data` and `bls_multi_operator` in isolated mode and confirm both pass |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
