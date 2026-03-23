# Phase 8: Integration and Verification - Research

**Researched:** 2026-03-20
**Domain:** E2E test infrastructure, BLS contract deployment, EIP-2537 precompiles, multi-operator testing
**Confidence:** HIGH

## Summary

Phase 8 requires two distinct deliverables: (1) a new BLS E2E test that deploys poa-middleware BLS contracts on local anvil, registers multiple operators with BLS keys, triggers a BLS service, and verifies the aggregated BLS signature is accepted on-chain; and (2) confirmation that all existing secp256k1 E2E tests pass without modification.

The existing E2E test infrastructure in `packages/layer-tests/` is sophisticated -- it uses a test matrix/registry pattern where each test is defined via `TestBuilder`, deploys its own service manager via Docker-based poa-middleware containers, registers operators with signing keys, deploys trigger and submit contracts, then fires triggers and validates output. The multi-operator test (`evm_multi_operator`) already exercises P2P mesh formation, quorum detection, and multi-node WAVS instances. The BLS test builds on this exact pattern but requires several critical adaptations:

1. **Anvil must run with `--hardfork prague`** -- The BLS contracts use EIP-2537 precompiles (addresses 0x0b-0x11) which are only available in the Prague hardfork. The current `LameAnvilInstanceBuilder` does NOT pass this flag.
2. **A BLS-specific poa-middleware deployment path is needed** -- The Docker image `ghcr.io/lay3rlabs/poa-middleware:1.0.1` only builds ECDSA contracts (FOUNDRY_PROFILE=ecdsa). BLS deployment requires `cli.sh -s bls deploy` which sources `scripts/bls/foundry_profile.sh` (FOUNDRY_PROFILE=bls) and uses `evm_version = "prague"`.
3. **BLS operator registration uses `updateOperatorSigningKey(bytes blsKey, bytes blsSigProof)`** instead of the ECDSA `updateOperatorSigningKey(address signingKey, bytes signature)`. The BLS proof is a BLS signature over `keccak256(abi.encode(operator_address))`.
4. **The submit contract (`SimpleSubmit.sol`) uses the ECDSA `IWavsServiceHandler`** -- for BLS tests, a BLS-compatible service handler contract is needed that imports the BLS `IWavsServiceHandler` with `SignatureData { signerPubkeys, aggregateSignature, referenceBlock }`.
5. **`SignatureKind::bls_default()`** must be used instead of `SignatureKind::evm_default()` when creating the BLS service's submit configuration.

**Primary recommendation:** Add a new `EvmService::BlsMultiOperator` variant and `EvmMiddlewareType::PoaBls` to the test infrastructure, with a parallel `PoaBlsMiddleware` implementation that uses `cli.sh -s bls` commands. Anvil must be configurable to use the Prague hardfork. A BLS-compatible `SimpleSubmit` contract needs to be created or the existing one adapted.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INT-01 | E2E test: BLS service on local anvil with poa-middleware BLS contracts, multi-operator quorum reached and verified on-chain | Existing multi-operator test pattern + BLS middleware + Prague anvil + BLS operator registration + BLS submit contract |
| INT-02 | Existing secp256k1 e2e tests unchanged and still passing | Secp256k1 tests do not use Prague hardfork or BLS middleware -- changes must be additive only (new enum variants, conditional hardfork flag) |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| blst | 0.3.16 | BLS key derivation for test operator registration (sign proof-of-possession) | Already in workspace; needed to create BLS signing key proofs for `updateOperatorSigningKey` |
| alloy-primitives | (workspace) | keccak256 for BLS key ownership proof, Address types | Already used throughout tests |
| alloy-provider | (workspace) | Anvil API, contract deployment, transaction submission | Already used in all EVM tests |
| alloy-sol-macro | (workspace) | BLS submit contract ABI bindings | Already used for SimpleSubmit bindings |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| const-hex | (workspace) | Hex encoding of BLS pubkeys/signatures for Docker CLI calls | Already in layer-tests dependencies |
| tokio | (workspace) | Async test execution, process management | Already used |
| tempfile | (workspace) | Temp directories for Docker-based contract deployment | Already used |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Docker-based BLS deployment via poa-middleware image | Direct Rust-based forge script execution | Docker approach is consistent with existing ECDSA tests; direct execution would break pattern |
| New BLS SimpleSubmit contract | Modify existing SimpleSubmit to be algorithm-agnostic | Separate contract is safer -- no risk of breaking ECDSA tests (INT-02) |

**Installation:**
No new dependencies needed. All crates already in workspace.

## Architecture Patterns

### Recommended Changes

```
packages/layer-tests/src/
  e2e/
    matrix.rs                     # Add EvmService::BlsMultiOperator variant
    test_registry.rs              # Add register_evm_bls_multi_operator_test()
    config.rs                     # BLS test triggers multi-operator mode
    handles/evm.rs                # Pass --hardfork prague conditionally
    helpers.rs                    # BLS-aware create_submit_from_config
    service_managers.rs           # BLS operator registration path (BLS keys instead of EVM addresses)

packages/utils/src/test_utils/middleware/evm/
    common.rs                     # Add EvmMiddlewareType::PoaBls, BLS Docker image constant
    middleware_poa_bls.rs (new)   # PoaBlsMiddleware: deploy via cli.sh -s bls, configure with BLS keys

examples/contracts/solidity/mocks/
    SimpleBlsSubmit.sol (new)     # BLS-compatible service handler (imports BLS IWavsServiceHandler)
```

### Pattern 1: BLS Test Registration

**What:** Register a new BLS E2E test in the test matrix, similar to existing `evm_multi_operator` but with BLS signature algorithm
**When to use:** When the BLS test is included in the test matrix

```rust
// In test_registry.rs:
fn register_evm_bls_multi_operator_test(&mut self, chain: &ChainKey) -> &mut Self {
    self.register(
        TestBuilder::new("evm_bls_multi_operator")
            .with_description("Tests BLS multi-operator quorum with P2P and on-chain verification")
            .add_workflow(
                WorkflowId::new("bls_multi_operator_echo").unwrap(),
                WorkflowBuilder::new()
                    .with_operator_component(OperatorComponent::EchoData)
                    .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                    .with_trigger(TriggerDefinition::NewEvmContract(
                        EvmTriggerDefinition::SimpleContractEvent { chain: chain.clone() },
                    ))
                    .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator_bls(chain)))
                    .with_input_data(InputData::Text("bls-multi-operator test".to_string()))
                    .with_expected_output(ExpectedOutput::Text("bls-multi-operator test".to_string()))
                    .build(),
            )
            .with_service_manager_chain(chain)
            .with_multi_operator()
            .with_group(TestGroupId::P2p)
            .build(),
    )
}
```

### Pattern 2: BLS-Aware Anvil Startup

**What:** Pass `--hardfork prague` to anvil when BLS tests are in the test matrix
**When to use:** When any BLS test is enabled

```rust
// In handles/evm.rs, LameAnvilInstanceBuilder::spawn():
let mut args = vec![
    "-p".to_string(), self.port.to_string(),
    "--chain-id".to_string(), self.chain_id,
    "--block-time".to_string(), "1".to_string(),
    "--order".to_string(), "fifo".to_string(),
    "--block-base-fee-per-gas".to_string(), "0".to_string(),
    "--gas-price".to_string(), "0".to_string(),
    "--disable-block-gas-limit".to_string(),
];

// BLS tests require EIP-2537 precompiles (Prague hardfork)
if self.hardfork_prague {
    args.push("--hardfork".to_string());
    args.push("prague".to_string());
}
```

**CRITICAL NOTE:** Prague hardfork in anvil should be backward-compatible with secp256k1 tests since Prague is a superset of previous hardforks. However, this must be verified. The safest approach is to always use Prague (since it is now the default EVM version in Foundry v1.0+). If any secp256k1 test fails with Prague, then conditional hardfork selection based on test matrix is needed.

### Pattern 3: BLS Operator Registration via Docker

**What:** Register BLS operators using poa-middleware Docker container's BLS scripts
**When to use:** When deploying a BLS service manager for test

```rust
// BLS operator registration flow (in PoaBlsMiddleware):
// 1. Deploy: cli.sh -s bls deploy
// 2. Register operator: cli.sh -s bls owner_operation registerOperator <addr> <weight>
// 3. Update BLS signing key: cli.sh -s bls update_signing_key
//    Requires: OPERATOR_KEY, BLS_PUBKEY (128-byte G1 hex), BLS_SIG_PROOF (256-byte G2 hex)
//
// The BLS_SIG_PROOF is: bls_sign(keccak256(abi.encode(operator_address)))
// This proves the operator controls the BLS private key
```

### Pattern 4: BLS Submit Contract

**What:** A `SimpleBlsSubmit.sol` that imports the BLS `IWavsServiceHandler` and `IWavsServiceManager`
**When to use:** As the submission handler for BLS tests

The BLS service handler has a different `SignatureData` struct:
```solidity
// BLS IWavsServiceHandler.SignatureData:
struct SignatureData {
    bytes[] signerPubkeys;      // Array of 128-byte G1 public keys
    bytes aggregateSignature;   // Single 256-byte G2 aggregate signature
    uint32 referenceBlock;
}

// vs ECDSA IWavsServiceHandler.SignatureData:
struct SignatureData {
    address[] signers;
    bytes[] signatures;
    uint32 referenceBlock;
}
```

The BLS submit contract must import from the BLS interface path and call the BLS service manager's `validate()`.

### Pattern 5: BLS SignatureKind in Service Config

**What:** Use `SignatureKind::bls_default()` for BLS service workflows
**When to use:** When creating service submit configuration for BLS tests

```rust
// In helpers.rs, create_submit_from_config for BLS:
Ok(Submit::Aggregator {
    component: Box::new(component),
    signature_kind: SignatureKind::bls_default(),  // NOT evm_default()
})
```

### Anti-Patterns to Avoid
- **Modifying existing secp256k1 tests to accommodate BLS** -- BLS should be purely additive. INT-02 requires zero changes to existing tests.
- **Running secp256k1 tests through Prague hardfork without testing first** -- Prague should be backward-compatible but verify.
- **Using the same `SimpleSubmit.sol` for BLS** -- The BLS `IWavsServiceHandler` has a different `SignatureData` struct, so the existing contract won't compile against BLS interfaces.
- **Generating BLS keys in the Docker container** -- BLS key derivation must happen in Rust (using blst) to match how WAVS derives operator BLS keys from mnemonics. The hex-encoded G1 pubkey and G2 proof are passed as env vars to the Docker container.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BLS key derivation for test operators | Custom key generation | `bls_signing::derive_bls_key()` from `packages/utils` | Must match WAVS's HKDF-SHA256 derivation exactly |
| BLS proof-of-possession signing | Manual blst FFI | `bls_signing::bls_sign_digest()` from `packages/utils` | Must use correct DST matching HashToCurve.sol |
| EIP-2537 format conversion | Manual byte padding | Existing helpers in `packages/types/src/signing/signer.rs` | G1: 128 bytes, G2: 256 bytes with 16-byte zero padding per 48-byte coordinate |
| POA contract deployment | forge script from Rust | Docker container with `cli.sh -s bls` | Matches existing ECDSA test pattern, handles all deployment complexity |
| Multi-operator WAVS setup | Custom node spawning | Existing `AppHandles::start()` + `Configs` | Already handles P2P mesh, HD key derivation, port allocation |

**Key insight:** The test infrastructure is already designed for multi-operator testing with the `evm_multi_operator` test. The BLS test reuses 90% of this infrastructure -- the only differences are: (a) which middleware type deploys the service manager, (b) how operators register their signing keys (BLS G1 pubkey + proof instead of EVM address + signature), (c) which submit contract is used, and (d) what `SignatureKind` is set on the service.

## Common Pitfalls

### Pitfall 1: Anvil Missing Prague Hardfork
**What goes wrong:** BLS contracts deploy but all precompile calls return zero/fail silently. Signature verification passes vacuously or reverts with `PrecompileCallFailed`.
**Why it happens:** EIP-2537 precompiles (0x0b-0x11) only exist in Prague. Without `--hardfork prague`, calls to these addresses are treated as calls to empty addresses.
**How to avoid:** Always start anvil with `--hardfork prague` when BLS tests are in the matrix. Verify by checking that `BLS12381.g1Add()` returns non-zero output.
**Warning signs:** `PrecompileCallFailed(0x0b)` or `PrecompileCallFailed(0x0f)` reverts, or `InvalidBLSSignature` revert even with correct inputs.

### Pitfall 2: Docker Image Missing BLS Contracts
**What goes wrong:** `cli.sh -s bls deploy` fails because the Docker image was built only with `FOUNDRY_PROFILE=ecdsa`.
**Why it happens:** The current `poa-middleware:1.0.1` Dockerfile has `RUN FOUNDRY_PROFILE=ecdsa forge build` -- it does NOT build BLS contracts.
**How to avoid:** Either use a BLS-specific Docker image tag (if published), or build the poa-middleware Docker image locally with BLS support (`docker build --build-arg FOUNDRY_PROFILE=bls`), or deploy BLS contracts directly via forge from the submodule path.
**Warning signs:** `Error: No matching contract found` during deployment, or `cli.sh: Command deploy not found` in BLS mode.

### Pitfall 3: BLS Key Ownership Proof Mismatch
**What goes wrong:** `updateOperatorSigningKey` reverts with `InvalidBLSKeyOwnershipProof`.
**Why it happens:** The contract verifies `bls_verify(blsKey, HashToCurve(keccak256(abi.encode(operator_address))), blsSigProof)`. The Rust-side signing must use the same DST (`BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`) and the same message format (`keccak256(abi.encode(operator_address))`).
**How to avoid:** Use `bls_sign_digest()` from `packages/utils/src/bls_signing.rs` which already uses the correct DST. The message MUST be `keccak256(abi.encode(operator_address))` where `abi.encode` left-pads the address to 32 bytes (standard Solidity ABI encoding).
**Warning signs:** `InvalidBLSKeyOwnershipProof()` revert, `BLST_ERROR::BLST_VERIFY_FAIL`.

### Pitfall 4: referenceBlock Timing
**What goes wrong:** BLS signature validation reverts with `InvalidReferenceBlock` or `SignerNotRegistered`.
**Why it happens:** The contract requires `referenceBlock < block.number` at submission time, AND the BLS key must have been registered at or before `referenceBlock`. If operators register their BLS keys and then the test immediately triggers, the `referenceBlock` captured by the aggregator might be before key registration.
**How to avoid:** Ensure operator BLS key registration happens BEFORE the service is activated and triggers are sent. The existing test infrastructure does this correctly (register_operators runs before create_real_wavs_services).
**Warning signs:** `InvalidReferenceBlock()` or `SignerNotRegistered()` reverts.

### Pitfall 5: secp256k1 SignerResponse for BLS Services
**What goes wrong:** The `register_operators()` flow in `service_managers.rs` expects `SignerResponse::Secp256k1` and calls `evm_signer_address()`. For BLS services, `get_service_signer` returns `SignerResponse::Bls12381` with `g1_pubkey_hex` instead of `evm_address`.
**Why it happens:** The operator registration code was written for secp256k1 only.
**How to avoid:** The `register_operators()` method already has a match on `SignerResponse::Bls12381` (added in Phase 7), but it constructs `AvsOperator` with an EVM-style approach. For BLS, the entire registration path needs to use BLS keys instead.
**Warning signs:** `assert_eq!` failure on signing address comparison, or `OperatorNotRegistered` reverts.

### Pitfall 6: Mixed Hardfork Compatibility
**What goes wrong:** Switching anvil to Prague hardfork breaks existing secp256k1 tests.
**Why it happens:** Unlikely but possible -- Prague introduces new opcodes, gas changes, or behavioral changes that affect existing tests.
**How to avoid:** Run existing secp256k1 tests with `--hardfork prague` first to verify compatibility. If issues arise, use conditional hardfork selection per test.
**Warning signs:** Pre-existing tests failing with gas-related errors or unexpected reverts when Prague is enabled.

## Code Examples

### BLS Key Derivation for Test Operators

```rust
// Source: packages/utils/src/bls_signing.rs (existing)
use utils::bls_signing::{derive_bls_key, bls_sign_digest, bls_g1_pubkey_bytes, bls_g2_signature_bytes};

// Derive BLS key from operator mnemonic + HD index
let bls_secret = derive_bls_key(operator_mnemonic, service_hd_index)?;

// Get G1 public key (128 bytes, EIP-2537 format)
let g1_pubkey = bls_g1_pubkey_bytes(&bls_secret);

// Create proof-of-possession: sign keccak256(abi.encode(operator_address))
let message = keccak256(alloy_sol_types::abi::encode(&[operator_address.into_word()]));
let g2_sig_proof = bls_sign_digest(&bls_secret, message.as_slice())?;
let g2_proof_bytes = bls_g2_signature_bytes(&g2_sig_proof);

// Pass to Docker: BLS_PUBKEY=0x{g1_hex}, BLS_SIG_PROOF=0x{g2_hex}
```

### Docker BLS Deployment

```rust
// Source: Adapted from middleware_poa.rs for BLS
// Deploy: cli.sh -s bls deploy
Command::new("docker")
    .args([
        "exec",
        "-e", &format!("FUNDED_KEY={}", deployer_key_hex),
        "-e", &format!("RPC_URL={}", rpc_url),
        "-e", "DEPLOY_ENV=LOCAL",
        &container_id,
        "/wavs/scripts/cli.sh",
        "-s", "bls",  // <-- KEY DIFFERENCE: use BLS scripts
        "deploy",
    ])

// Register operator: cli.sh -s bls owner_operation registerOperator
// Update BLS signing key: cli.sh -s bls update_signing_key
```

### BLS Submit Contract (SimpleBlsSubmit.sol concept)

```solidity
// Source: Adapted from examples/contracts/solidity/mocks/SimpleSubmit.sol
// Key change: import BLS IWavsServiceHandler instead of ECDSA version

import {IWavsServiceHandler} from "poa-middleware/bls/interfaces/IWavsServiceHandler.sol";
import {IWavsServiceManager} from "poa-middleware/bls/interfaces/IWavsServiceManager.sol";

contract SimpleBlsSubmit is IWavsServiceHandler, ISimpleSubmit {
    IWavsServiceManager private immutable _SERVICE_MANAGER;

    function handleSignedEnvelope(
        IWavsServiceHandler.Envelope calldata envelope,
        IWavsServiceHandler.SignatureData calldata signatureData  // BLS variant
    ) external {
        _SERVICE_MANAGER.validate(envelope, signatureData);
        // ... same storage logic as SimpleSubmit
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| secp256k1-only E2E tests | BLS + secp256k1 dual E2E tests | v1.1 (this milestone) | Proves BLS pipeline works end-to-end on real contracts |
| ECDSA-only poa-middleware Docker | BLS-aware poa-middleware deployment | v1.1 | Requires Docker image with BLS contracts or local forge deployment |
| Default anvil hardfork | Prague hardfork for EIP-2537 | v1.1 | Enables on-chain BLS precompile verification |

**Deprecated/outdated:**
- Anvil without `--hardfork prague`: Cannot verify BLS signatures on-chain without Prague precompiles

## Open Questions

1. **Does `poa-middleware:1.0.1` Docker image include BLS contracts?**
   - What we know: The Dockerfile runs `FOUNDRY_PROFILE=ecdsa forge build` -- only ECDSA contracts are built. BLS requires `FOUNDRY_PROFILE=bls`.
   - What's unclear: Whether a BLS-specific Docker image tag exists (e.g., `:bls-1.0.1` or `:1.1.0`)
   - Recommendation: Check if a BLS image is published. If not, build locally from `contracts/poa-middleware/` with `FOUNDRY_PROFILE=bls`, or deploy BLS contracts directly via `forge script` from the submodule. The cleanest approach for tests is to create a `PoaBlsMiddleware` that uses Docker exec with `-s bls` flag, assuming the image includes both ECDSA and BLS build outputs (or a new image is published).

2. **Does Prague hardfork break any existing secp256k1 tests?**
   - What we know: Prague is a superset of Shanghai/Cancun. The Prague hardfork adds EIP-2537, EIP-7702, and other features but should not remove or change existing functionality.
   - What's unclear: Whether any gas calculation changes or EVM behavioral changes in Prague affect the existing test suite.
   - Recommendation: Test by running existing suite with `--hardfork prague` flag before adding it unconditionally. Foundry v1.0+ uses Prague as default, suggesting compatibility is expected.

3. **BLS submit contract deployment path**
   - What we know: The existing `SimpleSubmit.sol` is compiled as part of the WAVS examples contracts (`just solidity-build`). A BLS variant needs the BLS `IWavsServiceHandler` interface.
   - What's unclear: Whether to add the BLS submit contract to `examples/contracts/` (requires BLS interface imports) or to the poa-middleware Docker container
   - Recommendation: Create `SimpleBlsSubmit.sol` in `examples/contracts/solidity/mocks/` with the BLS interface vendored from poa-middleware. Build it with `evm_version = "prague"` in a separate foundry profile, or inline the BLS `IWavsServiceHandler` interface since it's just a Solidity interface file (no precompile dependency for the interface itself).

4. **How to pass BLS keys during operator registration**
   - What we know: The existing test registration flow derives EVM signing keys and passes them via `AvsOperator::with_keys()`. BLS registration needs G1 pubkey bytes and a G2 proof-of-possession.
   - What's unclear: Whether to extend `AvsOperator` with BLS fields or create a separate `BlsAvsOperator` struct
   - Recommendation: Extend `AvsOperator` with optional BLS fields (`bls_pubkey: Option<Vec<u8>>`, `bls_proof: Option<Vec<u8>>`) or create a `BlsOperatorInfo` that the BLS middleware accepts.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) |
| Config file | `packages/layer-tests/layer-tests.toml` |
| Quick run command | `cargo test -p layer-tests -- --nocapture` with `mode = { "isolated" = [{ evm = "bls_multi_operator" }] }` |
| Full suite command | `cargo test -p layer-tests` with `mode = "all"` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INT-01 | BLS E2E: deploy BLS contracts, register BLS operators, trigger, verify aggregated signature on-chain | e2e | `cargo test -p layer-tests` with isolated `bls_multi_operator` | No -- Wave 0 |
| INT-02 | Existing secp256k1 E2E tests unchanged and passing | e2e | `cargo test -p layer-tests` with isolated `echo_data`, `multi_operator` | Yes (existing) |

### Sampling Rate
- **Per task commit:** `cargo build -p layer-tests`
- **Per wave merge:** `cargo test -p layer-tests` with BLS test isolated
- **Phase gate:** Full `mode = "all"` test suite including both BLS and secp256k1 tests

### Wave 0 Gaps
- [ ] `examples/contracts/solidity/mocks/SimpleBlsSubmit.sol` -- BLS service handler contract
- [ ] `packages/utils/src/test_utils/middleware/evm/middleware_poa_bls.rs` -- BLS poa-middleware integration
- [ ] `packages/layer-tests/src/e2e/handles/evm.rs` -- Prague hardfork flag support
- [ ] `packages/layer-tests/src/e2e/matrix.rs` -- `EvmService::BlsMultiOperator` variant
- [ ] `packages/layer-tests/src/e2e/test_registry.rs` -- BLS test registration
- [ ] `packages/layer-tests/src/e2e/service_managers.rs` -- BLS operator registration path
- [ ] `packages/layer-tests/src/e2e/helpers.rs` -- BLS-aware submit config creation

## Sources

### Primary (HIGH confidence)
- **packages/layer-tests/ source code** -- Direct reading of all E2E test infrastructure files (test_registry.rs, service_managers.rs, runner.rs, config.rs, helpers.rs, handles/evm.rs, matrix.rs, test_definition.rs, components.rs)
- **contracts/poa-middleware/ source code** -- Direct reading of BLS POAStakeRegistry.sol, BLS12381.sol, deployment scripts (cli.sh, bls/deploy.sh, bls/update_signing_key.sh, bls/owner_operation.sh), foundry.toml profiles, Dockerfile
- **Phase 7 summaries** (07-01-SUMMARY.md, 07-02-SUMMARY.md) -- BLS aggregation implementation details, file modifications, patterns established
- **Phase 7 research** (07-RESEARCH.md) -- BLS format conversion patterns, contract ABI details, pitfall documentation
- **packages/utils/src/test_utils/middleware/evm/** -- MockEvmServiceManager, PoaMiddleware implementation, MiddlewareServiceManagerConfig

### Secondary (MEDIUM confidence)
- [Foundry v1.0 announcement](https://www.paradigm.xyz/2025/02/announcing-foundry-v1-0) -- Prague as default EVM version, anvil hardfork support
- [Anvil CLI reference](https://getfoundry.sh/anvil/reference/anvil/) -- `--hardfork` flag documentation
- [EIP-2537 specification](https://eips.ethereum.org/EIPS/eip-2537) -- Precompile addresses, input/output formats

### Tertiary (LOW confidence)
- Whether `poa-middleware:1.0.1` Docker image includes BLS compiled artifacts (not verified -- Dockerfile only shows ECDSA build)

## Metadata

**Confidence breakdown:**
- Test infrastructure: HIGH - Direct code reading of all relevant files
- BLS contract deployment: HIGH - Read poa-middleware source, scripts, Dockerfile, foundry.toml
- EIP-2537/Prague requirement: HIGH - BLS12381.sol explicitly uses precompile addresses 0x0b-0x11, foundry.toml uses `evm_version = "prague"`
- Docker image BLS support: LOW - Dockerfile only shows ECDSA build; need to verify if published image includes BLS
- Pitfalls: HIGH - Identified from direct code reading and contract analysis

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable -- test infrastructure and poa-middleware contracts are fixed)
