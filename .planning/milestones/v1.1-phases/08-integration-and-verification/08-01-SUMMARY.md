---
phase: 08-integration-and-verification
plan: 01
subsystem: testing
tags: [bls, e2e, forge, anvil, prague, poa-middleware, solidity]

# Dependency graph
requires:
  - phase: 07-bls-aggregation
    provides: BLS aggregation and submission pipeline
provides:
  - PoaBlsMiddleware for deploying BLS poa-middleware contracts via local forge
  - Prague hardfork flag on anvil for EIP-2537 BLS precompiles
  - SimpleBlsSubmit.sol BLS-compatible service handler contract
  - BLS key fields (G1 pubkey, G2 proof) on AvsOperator
  - EvmMiddlewareType::PoaBls and EvmMiddleware::PoaBls variants
affects: [08-02-PLAN, integration-tests, bls-e2e]

# Tech tracking
tech-stack:
  added: []
  patterns: [local-forge-deployment, cast-send-for-contract-interaction, env-var-foundry-profile]

key-files:
  created:
    - packages/utils/src/test_utils/middleware/evm/middleware_poa_bls.rs
    - examples/contracts/solidity/interfaces/bls/IWavsServiceHandler.sol
    - examples/contracts/solidity/interfaces/bls/IWavsServiceManager.sol
    - examples/contracts/solidity/mocks/SimpleBlsSubmit.sol
  modified:
    - packages/utils/src/test_utils/middleware/operator.rs
    - packages/utils/src/test_utils/middleware/evm/common.rs
    - packages/utils/src/test_utils/middleware/evm/mod.rs
    - packages/layer-tests/src/e2e/handles/evm.rs
    - packages/layer-tests/src/e2e/handles.rs

key-decisions:
  - "PoaBlsMiddleware uses local forge/cast instead of Docker image -- avoids uncertainty about Docker image BLS support"
  - "FOUNDRY_PROFILE=bls env var instead of --profile flag for forge commands"
  - "SimpleBlsSubmit does not implement ISimpleSubmit -- BLS SignatureData is incompatible with ECDSA SignatureData struct"
  - "BLS tests verify via isValidTriggerId() after validate() pairing check passes"
  - "hardfork_prague parameter added to EvmInstance::spawn with false default for backward compatibility"

patterns-established:
  - "Local forge deployment: PoaBlsMiddleware resolves poa-middleware submodule path relative to workspace_path() parent"
  - "Cast send for operator registration: direct CLI interaction without Docker exec"

requirements-completed: [INT-01]

# Metrics
duration: 6min
completed: 2026-03-20
---

# Phase 8 Plan 01: BLS Test Infrastructure Summary

**PoaBlsMiddleware deploying BLS contracts via local forge, Prague-capable anvil, SimpleBlsSubmit.sol, and BLS-aware AvsOperator**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-20T12:45:31Z
- **Completed:** 2026-03-20T12:52:16Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Created PoaBlsMiddleware that builds and deploys BLS poa-middleware contracts using local forge script and cast send (no Docker dependency)
- Added Prague hardfork flag to anvil builder enabling EIP-2537 BLS precompiles
- Vendored BLS IWavsServiceHandler and IWavsServiceManager interfaces into examples/contracts/solidity/interfaces/bls/
- Created SimpleBlsSubmit.sol contract that validates BLS aggregate signatures via service manager
- Extended AvsOperator with optional bls_pubkey (128-byte G1) and bls_proof (256-byte G2) fields
- Integrated PoaBls into EvmMiddlewareType and EvmMiddleware enum with full dispatch

## Task Commits

Each task was committed atomically:

1. **Task 1: BLS AvsOperator fields, Prague anvil, and SimpleBlsSubmit contract** - `144342a5` (feat)
2. **Task 2: PoaBlsMiddleware using local forge and EvmMiddleware integration** - `9d37e4de` (feat)

## Files Created/Modified
- `packages/utils/src/test_utils/middleware/evm/middleware_poa_bls.rs` - PoaBlsMiddleware with deploy/configure/set_uri using forge/cast
- `packages/utils/src/test_utils/middleware/evm/common.rs` - PoaBls variants in EvmMiddlewareType and EvmMiddleware enums
- `packages/utils/src/test_utils/middleware/evm/mod.rs` - middleware_poa_bls module registration
- `packages/utils/src/test_utils/middleware/operator.rs` - BLS key fields and with_bls_keys constructor on AvsOperator
- `packages/layer-tests/src/e2e/handles/evm.rs` - hardfork_prague field and --hardfork prague args
- `packages/layer-tests/src/e2e/handles.rs` - Updated EvmInstance::spawn call with false default
- `examples/contracts/solidity/interfaces/bls/IWavsServiceHandler.sol` - BLS service handler interface (G1 pubkeys, G2 aggregate sig)
- `examples/contracts/solidity/interfaces/bls/IWavsServiceManager.sol` - BLS service manager interface with validate()
- `examples/contracts/solidity/mocks/SimpleBlsSubmit.sol` - BLS submission handler contract

## Decisions Made
- PoaBlsMiddleware uses local forge/cast instead of Docker image to avoid uncertainty about BLS artifact inclusion in Docker image
- FOUNDRY_PROFILE=bls environment variable used (not --profile flag) to match poa-middleware shell scripts
- SimpleBlsSubmit does not implement ISimpleSubmit because BLS SignatureData type is incompatible with ECDSA SignatureData; BLS tests verify via isValidTriggerId() instead
- hardfork_prague added as a parameter to EvmInstance::spawn rather than always-on, preserving backward compatibility for existing tests (Plan 02 will set it to true)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- PoaBlsMiddleware ready for Plan 02 to wire into E2E test matrix
- Prague anvil flag ready for Plan 02 to enable for BLS tests
- SimpleBlsSubmit.sol ready for deployment in BLS E2E test
- AvsOperator BLS fields ready for operator registration in BLS tests

## Self-Check: PASSED

All created files verified present. All task commits (144342a5, 9d37e4de) verified in git log.

---
*Phase: 08-integration-and-verification*
*Completed: 2026-03-20*
