---
phase: 08-integration-and-verification
plan: 02
status: complete
started: 2026-03-20T10:00:00.000Z
completed: 2026-03-23T12:00:00.000Z
one_liner: "BLS E2E test fully wired into test matrix with per-test middleware dispatch, verified in isolated and mixed modes"
---

## What Was Built

Wired the BLS E2E test (`evm_bls_multi_operator`) into the WAVS test matrix end-to-end. The test exercises the full BLS pipeline: 3 operators with BLS signing keys, P2P broadcast, aggregated BLS signature, on-chain EIP-2537 pairing verification via `SimpleBlsSubmit` contract, and result validation.

Key infrastructure:
- `EvmService::BlsMultiOperator` variant in test matrix, selectable via `layer-tests.toml`
- Per-test middleware dispatch (`EvmMiddlewares` struct) — BLS tests use `PoaBls`, secp256k1 tests use `Poa`
- BLS operator registration with `blst`-derived G1 pubkeys and G2 proof-of-possession
- `SimpleBlsSubmit` contract deployment for BLS submission handler
- `SignatureKind::bls_default()` for BLS workflow configuration
- BLS-specific test verification path (`evm_wait_for_bls_trigger_validated`) since `SimpleBlsSubmit` uses `isValidTriggerId` not `getSignedData`
- `InsufficientQuorum` error decoding in BLS aggregator submit path (mirrors secp256k1 path)
- Removed unnecessary `--hardfork prague` flag (anvil 1.4.4 defaults to Prague)

## Verification Results

All three test modes pass:

| Mode | Tests | Status | Duration |
|------|-------|--------|----------|
| BLS isolated | `bls_multi_operator` | PASS | 1.86s |
| secp256k1 isolated | `echo_data` | PASS | 1.96s |
| Mixed | `echo_data` + `bls_multi_operator` | PASS | ~13s total |

INT-01 verified: BLS end-to-end on Prague anvil with EIP-2537 precompiles
INT-02 verified: secp256k1 regression-free even when BLS test coexists in matrix

## Key Decisions

- Used `isValidTriggerId` polling instead of `getSignedData` for BLS test verification (SimpleBlsSubmit doesn't implement the latter)
- Removed Prague hardfork flag — anvil 1.4.4 defaults to Prague, making the flag redundant and eliminating a false regression signal
- Per-test middleware dispatch via `EvmMiddlewares` struct rather than global middleware type

## Issues Encountered

- G2 coordinate ordering: BLS signatures use c0|c1 order for EIP-2537, not c1|c0 (fixed in earlier commit)
- Prague hardfork "regression" was misdiagnosed — anvil already defaults to Prague, issue was unrelated
- BLS operator funding: operators needed ETH before registration (added self-funding step)

## Key Files

### Created
- `examples/contracts/solidity/mocks/SimpleBlsSubmit.sol` — BLS submission contract

### Modified
- `packages/layer-tests/src/e2e/matrix.rs` — `BlsMultiOperator` variant
- `packages/layer-tests/src/e2e/test_registry.rs` — BLS test registration
- `packages/layer-tests/src/e2e/test_definition.rs` — `bls: bool` field
- `packages/layer-tests/src/e2e/service_managers.rs` — BLS operator registration dispatch
- `packages/layer-tests/src/e2e/helpers.rs` — BLS submit deployment, `evm_wait_for_bls_trigger_validated`
- `packages/layer-tests/src/e2e/handles.rs` — `EvmMiddlewares` struct, per-test dispatch
- `packages/layer-tests/src/e2e/handles/evm.rs` — Removed Prague hardfork flag
- `packages/layer-tests/src/e2e/runner.rs` — BLS verification branch
- `packages/layer-tests/src/example_evm_client/solidity_types.rs` — `SimpleBlsSubmit` bindings
- `packages/wavs/src/subsystems/aggregator/submit.rs` — BLS `InsufficientQuorum` decoding

## Self-Check

- [x] BLS E2E test passes in isolation
- [x] secp256k1 tests pass unchanged (INT-02)
- [x] Mixed mode passes (both BLS + secp256k1)
- [x] Per-test middleware dispatch works correctly
- [x] BLS operator registration with blst keys
- [x] SimpleBlsSubmit contract deployment
- [x] SignatureKind::bls_default() used for BLS workflows
- [x] Code compiles cleanly
