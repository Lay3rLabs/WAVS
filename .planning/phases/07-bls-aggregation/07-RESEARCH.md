# Phase 7: BLS Aggregation - Research

**Researched:** 2026-03-20
**Domain:** BLS12-381 signature aggregation, EVM on-chain submission
**Confidence:** HIGH

## Summary

Phase 7 implements BLS signature aggregation in the WAVS aggregator subsystem. The existing aggregator already collects operator submissions into a quorum queue, attempts on-chain submission, and handles retry logic. This phase fills in four specific gaps left by Phase 5 and 6: (1) the `signature_data()` BLS arm that currently calls `unimplemented!()`, (2) the `SignatureData::Bls12381 -> ServiceManagerSignatureData` conversion, (3) a new `send_bls_envelope_signatures()` method that calls the BLS service handler contract, and (4) the `append_submission_to_queue()` deduplication for BLS (which currently calls `evm_signer_address()` which returns `Err` for BLS).

The existing aggregator flow (quorum queue, retry on InsufficientQuorum, burn on success) is unchanged. The only changes needed are: (a) implement BLS aggregation logic in `signature_data()`, (b) add a BLS submission path alongside the existing secp256k1 path in `handle_action_submit_evm()`, (c) fix deduplication in `append_submission_to_queue()` to support BLS G1 pubkeys, and (d) wire the BLS service handler/manager contract bindings for RPC calls.

**Primary recommendation:** Implement BLS aggregation as a parallel path within the existing aggregator flow, touching only the four `unimplemented!/error` sites plus adding BLS-specific EVM contract interaction. No architectural changes to the aggregator's queue, retry, or P2P logic.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| AGG-01 | Aggregator collects BLS submissions, accumulates G2 sigs and G1 pubkeys until quorum | Existing quorum queue + `append_submission_to_queue()` already accumulates; needs BLS dedup using G1 pubkey instead of EVM address |
| AGG-02 | G2 signatures aggregated via point addition; pubkeys sorted by keccak256 ascending | `blst::min_pk::AggregateSignature::aggregate()` for G2; `alloy_primitives::keccak256()` for sorting; contract enforces `lastKeyHash < keyHash` |
| AGG-03 | `referenceBlock` captured at quorum time, must be < submission block | Existing pattern: `provider.get_block_number().await - 1` already used in `handle_action_submit_evm()` |
| AGG-04 | Aggregated `SignatureData { signerPubkeys[], aggregateSignature, referenceBlock }` submitted to BLS service manager | BLS contract ABI already imported (`BlsServiceHandler`, `BlsServiceManager`); need RPC bindings + `send_bls_envelope_signatures()` |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| blst | 0.3.16 | BLS12-381 G2 signature aggregation (point addition) | Already in Cargo.lock; used by Phases 5-6 for signing; `AggregateSignature::aggregate()` for combining |
| alloy-primitives | (workspace) | `keccak256()` for pubkey sorting, `Bytes` for contract args | Already used throughout; sorting key is `keccak256(g1_pubkey_128_bytes)` |
| alloy-sol-macro | (workspace) | BLS contract RPC bindings (`#[sol(rpc)]`) | Needed to create callable contract instances (Phase 5 deferred RPC bindings) |
| alloy-provider | (workspace) | `DynProvider` for contract calls | Already used in secp256k1 submission path |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | (workspace) | `spawn_blocking` for CPU-bound BLS aggregation | If aggregating many signatures becomes expensive (may not be needed for typical quorum sizes) |
| tracing | (workspace) | Structured logging for aggregation events | Standard throughout codebase |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| blst `AggregateSignature::aggregate()` | Manual G2 point addition via blst FFI | `aggregate()` is simpler, handles group check; FFI only needed if performance profiling shows bottleneck |
| New `send_bls_envelope_signatures()` | Extending existing `send_envelope_signatures()` | Separate method is cleaner; BLS uses different contract ABI (BlsServiceHandler vs IWavsServiceHandler) |

**Installation:**
No new dependencies needed. All crates already in workspace.

## Architecture Patterns

### Recommended Changes (Minimal Diff)

```
packages/types/src/
  signing/signer.rs      # Implement BLS arm of signature_data()
  signing.rs             # Implement SignatureData::Bls12381 -> ServiceManagerSignatureData
  solidity_types/bls.rs  # Add #[sol(rpc)] variant for BLS bindings

packages/utils/src/
  evm_client/contracts.rs  # Add bls_service_handler() + bls_service_manager() helpers
  evm_client/signing.rs    # Add send_bls_envelope_signatures()

packages/wavs/src/subsystems/aggregator/
  queue.rs               # Fix append_submission_to_queue() BLS dedup
  submit.rs              # Add handle_action_submit_evm_bls() or dispatch in existing
```

### Pattern 1: BLS Signature Aggregation (signature_data() BLS arm)

**What:** Collect G2 signatures from queue, aggregate with blst, sort G1 pubkeys by keccak256
**When to use:** When `WavsSignature::Bls12381` is the first signature in the list

```rust
// In packages/types/src/signing/signer.rs, signature_data() BLS arm:
WavsSignature::Bls12381 { .. } => {
    // Collect (keccak256_hash, g1_pubkey_bytes, g2_signature) from each submission
    let mut entries: Vec<(FixedBytes<32>, Bytes, blst::min_pk::Signature)> = signatures
        .into_iter()
        .map(|sig| match sig {
            WavsSignature::Bls12381 { g2_signature, g1_pubkey, .. } => {
                let key_hash = keccak256(&g1_pubkey);
                let g2_sig = blst::min_pk::Signature::from_bytes(&compressed_from_eip2537_g2(&g2_signature))
                    .map_err(|e| SigningError::DataHash(anyhow::anyhow!("Invalid G2 sig: {:?}", e)))?;
                Ok((key_hash, Bytes::from(g1_pubkey), g2_sig))
            }
            WavsSignature::Secp256k1 { .. } => Err(SigningError::DataHash(
                anyhow::anyhow!("Mixed signature algorithms"),
            )),
        })
        .collect::<Result<_, _>>()?;

    // Sort by keccak256(pubkey) ascending -- contract enforces this
    entries.sort_by_key(|(hash, _, _)| *hash);

    // Aggregate G2 signatures via point addition
    let sig_refs: Vec<&blst::min_pk::Signature> = entries.iter().map(|(_, _, s)| s).collect();
    let aggregate = blst::min_pk::AggregateSignature::aggregate(&sig_refs, true)
        .map_err(|e| SigningError::DataHash(anyhow::anyhow!("BLS aggregate failed: {:?}", e)))?;
    let agg_sig_bytes = eip2537_from_compressed_g2(&aggregate.to_signature().serialize());

    let signer_pubkeys: Vec<Bytes> = entries.into_iter().map(|(_, pk, _)| pk).collect();

    Ok(SignatureData::Bls12381(BlsServiceHandler::SignatureData {
        signerPubkeys: signer_pubkeys,
        aggregateSignature: Bytes::from(agg_sig_bytes.to_vec()),
        referenceBlock: block_height as u32,
    }))
}
```

### Pattern 2: EIP-2537 Format Conversion

**What:** Convert between blst's compressed serialization (48-byte G1, 96-byte G2) and EIP-2537 uncompressed format (128-byte G1, 256-byte G2)
**When to use:** Whenever crossing the boundary between blst operations and on-chain data

**CRITICAL:** The signing path (Phase 6) already has helpers `bls_g2_signature_bytes()` (compressed -> EIP-2537) and `bls_g1_pubkey_bytes()`. For aggregation, we need the reverse: EIP-2537 -> compressed, to feed into blst's `Signature::from_bytes()`. Then convert back to EIP-2537 for the aggregate result.

```rust
// EIP-2537 G2 (256 bytes) -> blst compressed G2 (96 bytes)
fn compressed_from_eip2537_g2(eip2537: &[u8]) -> [u8; 96] {
    let mut compressed = [0u8; 96];
    // EIP-2537 pads each 48-byte coordinate to 64 bytes (16-byte zero prefix)
    for i in 0..4 {
        let src_offset = i * 64 + 16;  // skip 16-byte padding
        let dst_offset = i * 48;
        compressed[dst_offset..dst_offset + 48].copy_from_slice(&eip2537[src_offset..src_offset + 48]);
    }
    // Convert from serialized uncompressed to proper uncompressed format
    // Then use blst to reconstruct Signature
    compressed
}
// NOTE: The reverse (eip2537_from_compressed_g2) already exists in bls_g2_signature_bytes_inner
```

**IMPORTANT NOTE on serialization:** The blst `Signature::from_bytes()` expects **compressed** 96-byte G2 format, NOT uncompressed 192 bytes. But `Signature::serialize()` returns **uncompressed** 192 bytes. The signing code uses `serialize()` and then pads to EIP-2537 256 bytes. For aggregation we need to either:
- Store compressed form alongside EIP-2537 in `WavsSignature::Bls12381`, OR
- Use `Signature::uncompress()` from the 96-byte compressed form, OR
- Use `Signature::from_serialized()` (if it accepts 192-byte uncompressed), OR
- Strip EIP-2537 padding back to 192 bytes and use `Signature::deserialize()`

The correct approach: `Signature::deserialize(&uncompressed_192)` accepts uncompressed 192-byte G2 points. Extract the 192 bytes by stripping the EIP-2537 16-byte padding from each of the 4 coordinates.

```rust
// EIP-2537 G2 (256 bytes) -> blst uncompressed G2 (192 bytes)
fn uncompressed_from_eip2537_g2(eip2537: &[u8]) -> [u8; 192] {
    assert_eq!(eip2537.len(), 256);
    let mut uncompressed = [0u8; 192];
    for i in 0..4 {
        let src_offset = i * 64 + 16;
        let dst_offset = i * 48;
        uncompressed[dst_offset..dst_offset + 48].copy_from_slice(&eip2537[src_offset..src_offset + 48]);
    }
    uncompressed
}
```

Then: `blst::min_pk::Signature::deserialize(&uncompressed_192)` to get a `Signature` for aggregation.

### Pattern 3: BLS Queue Deduplication

**What:** The existing `append_submission_to_queue()` uses `evm_signer_address()` for dedup, which returns `Err` for BLS
**When to use:** Every time a BLS submission enters the queue

```rust
// Replace evm_signer_address with a generic signer_identity:
fn signer_identity(sig: &WavsSignature, envelope: &Envelope) -> Result<Vec<u8>, SigningError> {
    match sig {
        WavsSignature::Secp256k1 { .. } => {
            sig.evm_signer_address(envelope).map(|addr| addr.to_vec())
        }
        WavsSignature::Bls12381 { g1_pubkey, .. } => {
            // Use keccak256(g1_pubkey) as identity -- consistent with contract's sorting key
            Ok(keccak256(g1_pubkey).to_vec())
        }
    }
}
```

### Pattern 4: BLS EVM Submission Path

**What:** Call the BLS `IWavsServiceHandler.handleSignedEnvelope()` with BLS-typed `SignatureData`
**When to use:** When `SignatureData::Bls12381` is produced by the aggregation

The BLS service handler has the same `handleSignedEnvelope(Envelope, SignatureData)` interface but with different `SignatureData` fields. The BLS bindings need `#[sol(rpc)]` to generate callable contract instances.

```rust
// New method on EvmSigningClient (or in aggregator submit.rs):
pub async fn send_bls_envelope_signatures(
    &self,
    envelope: Envelope,  // Same Envelope type (shared between ECDSA and BLS)
    signature_data: BlsServiceHandler::SignatureData,
    service_handler: Address,
    max_gas: Option<u64>,
    gas_price: Option<u128>,
) -> Result<TransactionReceipt, EvmClientError> {
    // BLS service handler instance (from BLS ABI)
    let handler = BlsServiceHandler::IWavsServiceHandlerInstance::new(service_handler, self.provider.clone());
    // Envelope type: BLS handler uses the same Envelope struct (re-imported under bls ABI)
    let bls_envelope = BlsServiceHandler::Envelope {
        eventId: envelope.eventId,
        ordering: envelope.ordering,
        payload: envelope.payload,
    };
    // Call handleSignedEnvelope with BLS SignatureData
    handler.handleSignedEnvelope(bls_envelope, signature_data).send().await...
}
```

### Pattern 5: Dispatching Between Secp256k1 and BLS Submit

**What:** In `handle_action_submit_evm()`, dispatch on signature algorithm to choose submission path
**When to use:** At quorum time, before calling the contract

```rust
// In handle_action_submit_evm, after building signature_data:
match signature_data {
    SignatureData::Secp256k1(inner) => {
        // Existing path: validate + send via IWavsServiceHandler (secp256k1)
        client.send_envelope_signatures(...)
    }
    SignatureData::Bls12381(inner) => {
        // New path: validate + send via BlsServiceHandler
        client.send_bls_envelope_signatures(...)
    }
}
```

### Anti-Patterns to Avoid
- **Modifying the aggregator queue or retry logic:** The existing queue/retry/burn lifecycle works for both algorithms. Only the submission path and signature aggregation differ.
- **Adding BLS-specific quorum detection:** Quorum is determined by the contract's `validate()` call, not by the aggregator counting submissions. The aggregator just accumulates and tries to submit.
- **Storing compressed G2 alongside EIP-2537 in WavsSignature:** Would require a breaking serialization change. Instead, convert at aggregation time.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| G2 point addition | Manual EC math | `blst::min_pk::AggregateSignature::aggregate()` | Constant-time, audited, handles edge cases |
| G1 pubkey hash for sorting | Custom hash | `alloy_primitives::keccak256()` | Must match contract's `keccak256(pubkey)` exactly |
| EVM contract calls | Raw ABI encoding | Alloy `sol!` macro + `#[sol(rpc)]` | Type-safe, handles encoding/decoding/error types |
| Transaction retry logic | Custom retry | Existing `EvmSigningClient::send_envelope_signatures` retry pattern | Already handles nonce errors, exponential backoff |

**Key insight:** The aggregator's queue lifecycle (Active -> Burned), retry-on-failure, and P2P broadcast are all algorithm-agnostic. The only algorithm-specific code is: (1) how signatures are aggregated, (2) which contract ABI is called, and (3) how operator identity is determined for dedup.

## Common Pitfalls

### Pitfall 1: EIP-2537 vs Compressed Format Confusion
**What goes wrong:** blst uses different formats for serialization (compressed 48/96 bytes) vs the on-chain EIP-2537 format (128/256 bytes with zero-padding)
**Why it happens:** Phase 6 converts compressed -> EIP-2537 for signing. Phase 7 needs to reverse this for aggregation, then convert back.
**How to avoid:** Create explicit `eip2537_to_uncompressed_g2()` and `uncompressed_g2_to_eip2537()` helper functions. Use `Signature::deserialize()` (accepts 192-byte uncompressed), NOT `Signature::from_bytes()` (which expects 48-byte compressed).
**Warning signs:** `BLST_ERROR::BLST_BAD_ENCODING` when creating `Signature` objects from byte arrays.

### Pitfall 2: Sorting Mismatch Between Aggregator and Contract
**What goes wrong:** Contract reverts with `NotSorted()` or `InvalidSignatureOrder()`
**Why it happens:** Aggregator sorts pubkeys differently than the contract expects
**How to avoid:** Contract sorts by `keccak256(pubkey)` ascending (the 128-byte EIP-2537 G1 pubkey). Aggregator MUST sort the same way: `keccak256(g1_pubkey_128_bytes)` ascending, using raw bytes comparison of the `FixedBytes<32>`.
**Warning signs:** `NotSorted()` revert on `validate()` call.

### Pitfall 3: BLS Bindings Missing #[sol(rpc)]
**What goes wrong:** Cannot call BLS contract methods because bindings don't have RPC instances
**Why it happens:** Phase 5 explicitly used non-rpc bindings for BLS (`bls.rs` has no `#[sol(rpc)]`) to avoid import issues at the time
**How to avoid:** Add `#[sol(rpc)]` BLS bindings -- either in a new module in wavs-types (matching the rpc.rs pattern) or in packages/utils where the signing client already lives.
**Warning signs:** Compilation errors about missing `::new(address, provider)` on BLS contract types.

### Pitfall 4: ServiceManagerSignatureData Conversion Not Needed for BLS
**What goes wrong:** The existing secp256k1 path converts `SignatureData::Secp256k1 -> ServiceManagerSignatureData` for the `validate()` call. BLS uses a different `validate()` signature.
**Why it happens:** The secp256k1 `IWavsServiceManager.validate()` takes `ServiceManagerSignatureData` (with `signers: Address[], signatures: Bytes[]`). The BLS `IWavsServiceManager.validate()` takes `BlsServiceHandler::SignatureData` (with `signerPubkeys: Bytes[], aggregateSignature: Bytes`).
**How to avoid:** For BLS, bypass the `ServiceManagerSignatureData` conversion entirely. Call the BLS service manager's `validate()` directly with `BlsServiceHandler::SignatureData`. The `unimplemented!()` in `From<SignatureData> for ServiceManagerSignatureData` can remain or be removed, but it should never be hit for BLS flows.
**Warning signs:** Hitting the `unimplemented!()` panic at runtime.

### Pitfall 5: Queue Dedup Breaks for BLS
**What goes wrong:** `append_submission_to_queue()` calls `evm_signer_address()` which returns `Err` for BLS signatures, causing all BLS submissions to fail to enter the queue
**Why it happens:** `evm_signer_address()` explicitly returns error for `WavsSignature::Bls12381`
**How to avoid:** Refactor dedup to use a generic signer identity (G1 pubkey hash for BLS, EVM address for secp256k1)
**Warning signs:** "BLS signatures do not have EVM signer addresses" error in aggregator logs.

### Pitfall 6: BLS Envelope Type Mismatch
**What goes wrong:** The BLS `IWavsServiceHandler.Envelope` is a different Alloy-generated type from the secp256k1 `IWavsServiceHandler.Envelope`, even though they have identical fields
**Why it happens:** Alloy generates separate types per `sol!` macro invocation
**How to avoid:** Manually construct the BLS Envelope from the secp256k1 Envelope: `BlsServiceHandler::Envelope { eventId: envelope.eventId, ordering: envelope.ordering, payload: envelope.payload }`
**Warning signs:** Type mismatch compilation errors.

## Code Examples

### BLS G2 Signature Deserialization (from EIP-2537 format)

```rust
// Source: blst crate documentation + Phase 6 signing code (reverse operation)
fn deserialize_g2_from_eip2537(eip2537_bytes: &[u8]) -> Result<blst::min_pk::Signature, anyhow::Error> {
    if eip2537_bytes.len() != 256 {
        anyhow::bail!("Expected 256-byte EIP-2537 G2, got {}", eip2537_bytes.len());
    }
    // Strip 16-byte zero padding from each of the 4 coordinates
    let mut uncompressed = [0u8; 192];
    for i in 0..4 {
        let src_offset = i * 64 + 16;
        let dst_offset = i * 48;
        uncompressed[dst_offset..dst_offset + 48]
            .copy_from_slice(&eip2537_bytes[src_offset..src_offset + 48]);
    }
    blst::min_pk::Signature::deserialize(&uncompressed)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize G2 signature: {:?}", e))
}
```

### BLS G2 Aggregate Signature Serialization (back to EIP-2537)

```rust
// Source: Phase 6 bls_g2_signature_bytes_inner (same logic)
fn serialize_aggregate_to_eip2537(aggregate: &blst::min_pk::AggregateSignature) -> [u8; 256] {
    let sig = aggregate.to_signature();
    let uncompressed = sig.serialize(); // 192 bytes
    let mut eip2537 = [0u8; 256];
    for i in 0..4 {
        let src_offset = i * 48;
        let dst_offset = i * 64 + 16;
        eip2537[dst_offset..dst_offset + 48]
            .copy_from_slice(&uncompressed[src_offset..src_offset + 48]);
    }
    eip2537
}
```

### BLS Contract RPC Bindings

```rust
// Source: Existing pattern in rpc.rs for secp256k1, applied to BLS
// Add to packages/types/src/solidity_types/bls.rs or a new bls_rpc.rs

mod bls_service_handler_rpc {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IWavsServiceHandler,
        "./src/contracts/solidity/abi/bls/IWavsServiceHandler.json"
    );
}

mod bls_service_manager_rpc {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        #[derive(Debug)]
        IWavsServiceManager,
        "./src/contracts/solidity/abi/bls/IWavsServiceManager.json"
    );
}

// Re-export with Bls prefix for RPC types
pub use bls_service_handler_rpc::IWavsServiceHandler as BlsServiceHandlerRpc;
pub use bls_service_manager_rpc::IWavsServiceManager as BlsServiceManagerRpc;

pub type BlsServiceHandlerInstance = BlsServiceHandlerRpc::IWavsServiceHandlerInstance<DynProvider>;
pub type BlsServiceManagerInstance = BlsServiceManagerRpc::IWavsServiceManagerInstance<DynProvider>;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| secp256k1 only | BLS12-381 + secp256k1 dual-path | v1.1 (this milestone) | Enables aggregate signatures, reducing on-chain verification cost |
| Individual sig verification | Aggregate BLS verification (one pairing check) | BLS addition | Gas savings scale with operator count |

**Deprecated/outdated:**
- `ServiceManagerSignatureData` conversion for BLS: Not needed -- BLS uses its own contract interface directly

## Open Questions

1. **BLS RPC bindings location**
   - What we know: Phase 5 explicitly used non-rpc bindings (`bls.rs` has no `#[sol(rpc)]`). The decision note says "BLS contract interaction handled differently in Phase 7."
   - What's unclear: Whether to add `#[sol(rpc)]` alongside existing non-rpc bindings in bls.rs (behind a feature flag), or create a separate file, or put them in a different package
   - Recommendation: Follow the `rpc.rs`/`not_rpc.rs` pattern -- add `#[sol(rpc)]` BLS bindings conditionally behind the `solidity-rpc` feature flag. This is consistent with existing architecture.

2. **BLS validate() call path**
   - What we know: secp256k1 path calls `service_manager.validate()` then `service_handler.handleSignedEnvelope()`. BLS `IWavsServiceManager` also has `validate()`.
   - What's unclear: Whether to route through BLS service manager `validate()` first (matching secp256k1 pattern) or directly to handler
   - Recommendation: Mirror the secp256k1 pattern -- call BLS `service_manager.validate()` first, then `service_handler.handleSignedEnvelope()`. The BLS service manager's `validate()` delegates to `_checkSignatures()` on the stake registry.

3. **spawn_blocking for BLS aggregation**
   - What we know: blst operations are CPU-bound. Phase 6 uses `spawn_blocking` for signing.
   - What's unclear: Whether aggregation of typical quorum sizes (3-10 operators) is expensive enough to warrant `spawn_blocking`
   - Recommendation: Start without `spawn_blocking` since `AggregateSignature::aggregate()` over <10 signatures is fast. Add if profiling shows issues.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p wavs-types --features bls,signer,solidity-rpc -- bls` |
| Full suite command | `cargo build -p wavs -p wavs-types -p layer-utils && cargo test -p wavs-types --features full` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AGG-01 | BLS submissions enter quorum queue (dedup by G1 pubkey) | unit | `cargo test -p wavs -- aggregator::queue::bls` | No -- Wave 0 |
| AGG-02 | G2 aggregation + keccak256-sorted pubkeys | unit | `cargo test -p wavs-types --features bls,signer -- bls_signature_data` | No -- Wave 0 |
| AGG-03 | referenceBlock < current block | unit (existing pattern) | Covered by existing `handle_action_submit_evm` logic | Yes (existing) |
| AGG-04 | BLS SignatureData submitted to BLS contract | integration | `cargo test -p layer-utils --features bls -- bls_send` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo build -p wavs -p wavs-types -p layer-utils`
- **Per wave merge:** `cargo test -p wavs-types --features full && cargo test -p wavs`
- **Phase gate:** Full workspace build + test before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `packages/types/src/signing/signer.rs` -- unit test for BLS `signature_data()` aggregation (covers AGG-02)
- [ ] `packages/wavs/src/subsystems/aggregator/queue.rs` -- unit test for BLS dedup in `append_submission_to_queue()` (covers AGG-01)
- [ ] BLS format conversion helpers need unit tests for round-trip (EIP-2537 -> uncompressed -> aggregate -> EIP-2537)

## Sources

### Primary (HIGH confidence)
- **blst crate docs** (docs.rs/blst/0.3.16) - `AggregateSignature::aggregate()`, `Signature::deserialize()`, `Signature::serialize()` API
- **poa-middleware BLS contracts** (`contracts/src/bls/POAStakeRegistry.sol`) - `_checkSignatures()` validation logic, sorting requirement, `keccak256(pubkey)` key hash
- **BLS service handler ABI** (`packages/types/src/contracts/solidity/abi/bls/IWavsServiceHandler.json`) - `handleSignedEnvelope(Envelope, SignatureData)` signature
- **BLS service manager ABI** (`packages/types/src/contracts/solidity/abi/bls/IWavsServiceManager.json`) - `validate(Envelope, SignatureData)` signature
- **Existing aggregator code** (`packages/wavs/src/subsystems/aggregator/`) - Queue lifecycle, submit flow, retry mechanism
- **Phase 6 BLS signing code** (`packages/types/src/signing/signer.rs`) - EIP-2537 format helpers, `bls_sign_digest_inner()`, `bls_g2_signature_bytes_inner()`

### Secondary (MEDIUM confidence)
- [blst AggregateSignature docs](https://docs.rs/blst/latest/blst/min_pk/struct.AggregateSignature.html) - Verified API methods via web fetch

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all crates already in workspace, API verified via docs.rs
- Architecture: HIGH - building on existing aggregator patterns, only adding parallel BLS path
- Pitfalls: HIGH - identified from direct code reading (unimplemented sites, format conversions, contract ABI differences)
- EIP-2537 format: HIGH - verified by reading Phase 6 signing code and contract validation logic

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable -- blst 0.3.x, poa-middleware contracts fixed)
