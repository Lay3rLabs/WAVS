# Phase 6: BLS Signing Pipeline - Research

**Researched:** 2026-03-20
**Domain:** BLS12-381 signing, hash-to-curve, EIP-2537 encoding, async/blocking bridge
**Confidence:** HIGH

## Summary

Phase 6 replaces `unimplemented!()` stubs with actual BLS12-381 signing in the WAVS submission pipeline. The operator signs the `keccak256(abi_encode(envelope))` digest using BLS hash-to-curve, producing a G2 signature that is propagated over P2P alongside the operator's G1 public key. All existing secp256k1 code paths remain untouched.

The primary technical challenge is a **DST mismatch** between the commonware-cryptography library and the poa-middleware contracts. The contract uses DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_` while commonware's `Signer::sign()` trait uses DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_` (with `_POP_` suffix) and additionally wraps messages with `union_unique(namespace, message)` (varint-length-prefixed namespace concatenation). This means we must use the blst crate directly for signing -- specifically `blst::min_pk::SecretKey::sign()` -- rather than the commonware `Signer` trait. We keep `commonware_cryptography::bls12381::PrivateKey` for key derivation and storage, but extract the raw scalar bytes for blst signing.

A secondary concern is format conversion: commonware produces 96-byte compressed G2 signatures and 48-byte compressed G1 pubkeys, while the contract expects 256-byte and 128-byte EIP-2537 uncompressed format respectively. The G1 pubkey conversion already exists (`bls_g1_pubkey_bytes`); an analogous G2 signature conversion is needed.

**Primary recommendation:** Use `blst::min_pk::SecretKey::sign(msg, DST, &[])` directly for signing, with the contract-matching DST `b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_"`. Serialize the resulting signature to 192-byte uncompressed format via `Signature::serialize()`, then pad each 48-byte Fp element to 64 bytes for EIP-2537 (256 bytes total). Wrap the entire signing operation in `tokio::task::spawn_blocking` since blst is CPU-bound.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- `add_service_key` signature extended to accept `SignatureAlgorithm` (passed by caller)
- The dispatcher reads `service.signature_kind.algorithm` (from `Submit::Aggregator`) and passes it to `add_service_key`
- `add_service_key` creates `WavsCryptoSigner::Bls12381(bls_private_key_from_mnemonic(mnemonic, hd_index)?)` when `SignatureAlgorithm::Bls12381`
- Secp256k1 services: `WavsCryptoSigner::Secp256k1(make_signer(mnemonic, hd_index)?)` -- unchanged
- `bls` feature is **default-on** in `packages/types` Cargo.toml: add `bls` to `[features] default = [...]`
- `bls` feature is **default-on** in `packages/wavs` Cargo.toml: same
- BLS signing replaces `unimplemented!()` in `WavsCryptoSigner::Bls12381` arm of `WavsSigner::sign()`
- Signing input: `keccak256(abi_encode(envelope))` as 32 bytes
- blst call: `Sign::sign(&private_key, &digest_bytes, b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_")`
- blst is CPU-bound: wrap in `tokio::task::spawn_blocking`
- Returns `WavsSignature::Bls12381 { g2_signature: sig_bytes (256 bytes), g1_pubkey: pubkey_bytes (128 bytes), kind }`
- Zero behavioral changes to secp256k1 signing, signer creation, or P2P propagation

### Claude's Discretion
- SignerResponse BLS variant: not selected for discussion -- defer to Phase 8 or handle as Claude sees fit (stub or forward-looking variant)
- Error behavior for `0x` raw key with BLS service: handle at `add_service_key` time with clear error message
- Exact blst API call shape (Sign trait vs raw FFI) -- use whatever is cleanest with commonware-cryptography

### Deferred Ideas (OUT OF SCOPE)
- SignerResponse BLS variant for `/services/signer` HTTP endpoint -- not discussed; handle in Phase 6 as Claude sees fit, or defer to Phase 8
- MCP tooling for BLS operator registration -- explicitly deferred to v1.2 (per REQUIREMENTS.md)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SIGN-01 | Operator signs envelope digest with BLS key -> G2 signature (256 bytes) using hash-to-curve consistent with HashToCurve.sol (RFC 9380, DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`) | Direct blst signing via `blst::min_pk::SecretKey::sign()` with contract-matching DST; G2 uncompressed->EIP-2537 padding needed; `spawn_blocking` for CPU-bound operation |
| SIGN-02 | BLS signature and operator G1 pubkey included in Submission propagated over P2P | `WavsSignature::Bls12381 { g2_signature, g1_pubkey, kind }` variant already defined in Phase 5; serde roundtrip confirmed by existing tests |
| SIGN-03 | Existing secp256k1 signing path unchanged -- algorithm is per-service config | Secp256k1 code paths untouched; `add_service_key` dispatch by `SignatureAlgorithm` at creation time only |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| blst | 0.3.16 | Raw BLS12-381 signing with custom DST | Already in Cargo.lock; direct FFI gives control over DST and message format without commonware's namespace wrapping |
| commonware-cryptography | 2026.3.0 | BLS PrivateKey type for key storage and derivation | Already used for key derivation in Phase 5; `bls12381::PrivateKey` stored in `WavsCryptoSigner::Bls12381` |
| tokio | workspace | `spawn_blocking` for CPU-bound blst signing | Already used throughout WAVS for async operations |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| alloy-primitives | workspace | `keccak256`, `FixedBytes<32>` for digest computation | Already used in `WavsSignable::unprefixed_hash()` |
| commonware-codec | 2026.3.0 | `Encode` trait to extract raw bytes from `PrivateKey` | Getting 32-byte scalar from PrivateKey for blst SecretKey construction |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| blst direct | commonware `Signer::sign()` | Cannot use: commonware hardcodes DST `..._POP_` and wraps message with `union_unique()` -- incompatible with contract's DST `..._RO_` and raw message format |
| blst direct | commonware `ops::sign()` | Cannot use: requires `Private` which is a private field of `PrivateKey` -- not accessible from outside the crate |

## Architecture Patterns

### Recommended Project Structure
```
packages/utils/src/bls_signing.rs       # Add: bls_sign_digest(), bls_g2_signature_bytes()
packages/types/src/signing/signer.rs    # Modify: BLS arm in WavsSigner::sign()
packages/wavs/src/subsystems/submission.rs  # Modify: add_service_key() to accept SignatureAlgorithm
packages/wavs/src/dispatcher.rs         # Modify: pass SignatureAlgorithm to add_service_key()
packages/types/Cargo.toml               # Modify: bls feature default-on
packages/wavs/Cargo.toml                # Modify: bls feature default-on (if not already via wavs-types full)
```

### Pattern 1: blst Direct Signing (bypassing commonware Signer)

**What:** Extract raw 32-byte scalar from `commonware_cryptography::bls12381::PrivateKey`, construct `blst::min_pk::SecretKey`, call `sign()` with contract-matching DST.

**When to use:** When the signing DST must match the on-chain contract exactly and commonware's `Signer` trait uses an incompatible DST.

**Why this is necessary:** The contract `HashToCurve.sol` uses DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_` (line 20). The commonware library uses DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_` (with `_POP_` suffix, `group.rs` line 408). These produce different curve points for the same message, making the signatures incompatible. Additionally, commonware's `sign_message()` function wraps the message with `union_unique(namespace, message)` which prepends a varint-encoded namespace length -- the contract does not expect this wrapping.

**Example:**
```rust
// In packages/utils/src/bls_signing.rs

/// DST matching HashToCurve.sol line 20 -- NO _POP_ suffix.
const BLS_SIGNING_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_";

/// Sign a 32-byte digest using BLS12-381 hash-to-curve.
/// The digest is typically keccak256(abi_encode(envelope)).
/// Returns a 256-byte EIP-2537 uncompressed G2 signature.
pub fn bls_sign_digest(
    private_key: &commonware_cryptography::bls12381::PrivateKey,
    digest: &[u8; 32],
) -> anyhow::Result<[u8; 256]> {
    // Extract raw 32-byte scalar from PrivateKey via Encode trait
    use commonware_codec::Encode;
    let raw_bytes = private_key.encode();
    let sk = blst::min_pk::SecretKey::from_bytes(&raw_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to create blst SecretKey: {:?}", e))?;

    // Sign: hash-to-curve with contract-matching DST
    // The message is abi.encodePacked(digest) = raw 32 bytes
    let signature = sk.sign(digest, BLS_SIGNING_DST, &[]);

    // Convert to EIP-2537 uncompressed format (256 bytes)
    bls_g2_signature_bytes(&signature)
}
```

### Pattern 2: G2 Signature to EIP-2537 Format (256 bytes)

**What:** Convert blst's 192-byte uncompressed G2 serialization to 256-byte EIP-2537 format by padding each 48-byte Fp element to 64 bytes.

**When to use:** Whenever a G2 signature needs to match the contract's `BLS12381.G2_POINT_SIZE = 256`.

**Example:**
```rust
/// Convert a blst G2 signature to 256-byte EIP-2537 uncompressed format.
///
/// blst serializes G2 as 192 bytes: (x.c0[48] || x.c1[48] || y.c0[48] || y.c1[48])
/// EIP-2537 format: each Fp is 64 bytes (16 zero padding + 48 data) = 4 * 64 = 256 bytes
pub fn bls_g2_signature_bytes(
    signature: &blst::min_pk::Signature,
) -> anyhow::Result<[u8; 256]> {
    let uncompressed = signature.serialize(); // 192 bytes
    let mut eip2537 = [0u8; 256];

    // Pad each 48-byte Fp to 64 bytes (16 zero prefix + 48 data)
    // x.c0: uncompressed[0..48] -> eip2537[16..64]
    eip2537[16..64].copy_from_slice(&uncompressed[0..48]);
    // x.c1: uncompressed[48..96] -> eip2537[80..128]
    eip2537[80..128].copy_from_slice(&uncompressed[48..96]);
    // y.c0: uncompressed[96..144] -> eip2537[144..192]
    eip2537[144..192].copy_from_slice(&uncompressed[96..144]);
    // y.c1: uncompressed[144..192] -> eip2537[208..256]
    eip2537[208..256].copy_from_slice(&uncompressed[144..192]);

    Ok(eip2537)
}
```

### Pattern 3: Algorithm-Based Signer Creation in add_service_key

**What:** Extend `add_service_key` to accept `SignatureAlgorithm` and create the appropriate `WavsCryptoSigner` variant.

**When to use:** Service registration time -- the only point where algorithm selection matters.

**Example:**
```rust
pub fn add_service_key(
    &self,
    service_id: ServiceId,
    hd_index: Option<u32>,
    algorithm: SignatureAlgorithm,
) -> Result<(), SubmissionError> {
    let hd_index = hd_index.unwrap_or(/* ... atomic increment ... */);
    // ... counter update ...

    let signer = match algorithm {
        SignatureAlgorithm::Secp256k1 => {
            let pks = make_signer(&self.signing_mnemonic, Some(hd_index))?;
            WavsCryptoSigner::Secp256k1(pks)
        }
        SignatureAlgorithm::Bls12381 => {
            let bls_key = utils::bls_signing::bls_private_key_from_mnemonic(
                self.signing_mnemonic.as_str(),
                hd_index,
            )?;
            WavsCryptoSigner::Bls12381(bls_key)
        }
    };

    self.signers.write().unwrap().insert(
        service_id,
        SignerInfo { signer, hd_index },
    );
    Ok(())
}
```

### Pattern 4: spawn_blocking for BLS Signing

**What:** Wrap the CPU-bound blst signing in `tokio::task::spawn_blocking`.

**When to use:** Always when calling blst sign from async context.

**Example:**
```rust
#[cfg(feature = "bls")]
WavsCryptoSigner::Bls12381(ref bls_key) => {
    let hash = self.unprefixed_hash()?; // keccak256(abi_encode(envelope))
    let digest: [u8; 32] = hash.into();
    let key = bls_key.clone();
    let kind = kind.clone();

    let (g2_sig, g1_pub) = tokio::task::spawn_blocking(move || {
        let g2 = utils::bls_signing::bls_sign_digest(&key, &digest)?;
        let g1 = utils::bls_signing::bls_g1_pubkey_bytes(&key)?;
        Ok::<_, anyhow::Error>((g2, g1))
    })
    .await
    .map_err(|e| anyhow::anyhow!("BLS signing task failed: {e}"))??;

    Ok(WavsSignature::Bls12381 {
        g2_signature: g2_sig.to_vec(),
        g1_pubkey: g1_pub.to_vec(),
        kind,
    })
}
```

### Anti-Patterns to Avoid
- **Using commonware Signer::sign() for contract-compatible signatures:** The DST mismatch (`_POP_` vs `_RO_`) and `union_unique` message wrapping make it incompatible with on-chain verification. Always use blst directly with the contract DST.
- **Calling blst sign on the Tokio async runtime:** blst operations are CPU-bound and will block the runtime. Always use `spawn_blocking`.
- **Modifying any secp256k1 code paths:** SIGN-03 requires zero behavioral changes. The algorithm dispatch should happen only at `add_service_key` time and in the `WavsSigner::sign()` match arm.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BLS signing | Custom hash-to-curve | `blst::min_pk::SecretKey::sign()` | hash-to-curve is cryptographically subtle; blst implements RFC 9380 correctly |
| EIP-2537 padding | Manual byte manipulation | Reusable `bls_g2_signature_bytes` in utils | Same padding pattern as G1; centralize the conversion |
| Key derivation | Custom BLS keygen | `bls_private_key_from_mnemonic()` from Phase 5 | Already tested and proven deterministic |
| HD index management | Custom counter | Existing `signing_mnemonic_hd_index_count` AtomicU32 | Works for both secp256k1 and BLS; battle-tested |

**Key insight:** The only new cryptographic code needed is the bridge between commonware's PrivateKey format and blst's direct signing API. Everything else is plumbing.

## Common Pitfalls

### Pitfall 1: DST Mismatch with Contract
**What goes wrong:** Using commonware's `Signer::sign()` produces signatures with DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_` that fail on-chain verification in `HashToCurve.sol` which uses `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`.
**Why it happens:** The commonware library is designed for its own P2P consensus protocol which uses the POP scheme; the WAVS contracts follow the standard BLS signature scheme without the POP suffix.
**How to avoid:** Use `blst::min_pk::SecretKey::sign()` directly with `b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_"` as the DST.
**Warning signs:** Signatures verify fine in unit tests but fail on-chain. The verify call in POAStakeRegistry returns `InvalidBLSSignature`.

### Pitfall 2: union_unique Message Wrapping
**What goes wrong:** Even if you somehow match the DST, commonware's `sign_message()` wraps the message as `union_unique(namespace, message)` = `varint(namespace_len) || namespace || message`. The contract passes raw `abi.encodePacked(digest)` (32 bytes) with no namespace wrapping.
**Why it happens:** commonware's namespace scheme provides domain separation for multi-protocol usage; WAVS contracts have their own domain separation via `keccak256(abi.encode(envelope))`.
**How to avoid:** Pass the raw 32-byte digest directly to `blst::min_pk::SecretKey::sign()`.
**Warning signs:** Same as above -- signatures fail on-chain.

### Pitfall 3: G2 Signature Size (192 vs 256 bytes)
**What goes wrong:** blst `Signature::serialize()` produces 192-byte uncompressed G2 (4 x 48-byte Fp elements). Contract expects 256 bytes (4 x 64-byte EIP-2537 padded Fp elements). Passing 192 bytes to the contract reverts with `InvalidBLSSignatureLength`.
**Why it happens:** EIP-2537 padded format prepends 16 zero bytes to each 48-byte Fp element to make each 64 bytes (matching EVM word size).
**How to avoid:** Always pad via `bls_g2_signature_bytes()`. Assert output is exactly 256 bytes.
**Warning signs:** Contract revert `InvalidBLSSignatureLength` when `aggregateSignature.length != BLS12381.G2_POINT_SIZE`.

### Pitfall 4: PrivateKey Scalar Extraction
**What goes wrong:** `commonware_cryptography::bls12381::PrivateKey` stores the scalar as a private field `key: Private`. You cannot directly access it. Calling `Encode` gives the raw 32 bytes.
**Why it happens:** The `key` field is `pub(crate)` in commonware -- it's not accessible from external crates.
**How to avoid:** Use the `commonware_codec::Encode` trait: `private_key.encode()` returns the 32-byte scalar as `bytes::BytesMut`, which can be passed to `blst::min_pk::SecretKey::from_bytes()`.
**Warning signs:** Compilation error accessing `PrivateKey.key`.

### Pitfall 5: Feature Flag Ordering
**What goes wrong:** Adding `bls` to default features in `packages/types/Cargo.toml` but not ensuring `packages/wavs` gets it transitively, or vice versa.
**Why it happens:** `packages/wavs` depends on `wavs-types = { features = ["full"] }` which already includes `bls`. But `packages/types` default features only have `["cosmwasm"]` currently.
**How to avoid:** Add `bls` to default features in `packages/types/Cargo.toml`. Verify `wavs` already gets it via `full`. Check that no downstream crate breaks with `bls` being default-on.
**Warning signs:** Compilation errors about missing `Bls12381` variant when building with default features.

### Pitfall 6: Signing Digest Input Format
**What goes wrong:** Passing `keccak256(abi.encode(envelope))` as `FixedBytes<32>` to blst but the contract's `_checkSignatures` expects `abi.encodePacked(digest)` as the hash-to-curve input.
**Why it happens:** The contract flow is: `digest = keccak256(abi.encode(envelope))` then `hashToCurveG2(abi.encodePacked(digest))`. Since `digest` is `bytes32`, `abi.encodePacked(bytes32)` = raw 32 bytes (no padding). So the blst sign input is the raw 32-byte keccak hash.
**How to avoid:** Use `self.unprefixed_hash()` which returns `keccak256(abi_encode(envelope))` as `FixedBytes<32>`, then convert to `[u8; 32]` and pass directly to `bls_sign_digest()`.
**Warning signs:** Signatures verify in isolation but fail in contract `_checkSignatures`.

## Code Examples

### Complete BLS Signing Flow in bls_signing.rs
```rust
// Source: packages/utils/src/bls_signing.rs (to be added)

/// DST for BLS signing, matching HashToCurve.sol line 20.
/// NOTE: This is NOT the same as commonware's G2_MESSAGE DST which has _POP_ suffix.
pub const BLS_SIGNING_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_";

/// Sign a 32-byte digest with a BLS private key.
/// Returns 256-byte EIP-2537 G2 signature.
pub fn bls_sign_digest(
    private_key: &commonware_cryptography::bls12381::PrivateKey,
    digest: &[u8; 32],
) -> anyhow::Result<[u8; 256]> {
    use commonware_codec::Encode;
    let raw_bytes = private_key.encode();
    let sk = blst::min_pk::SecretKey::from_bytes(&raw_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to create blst SecretKey: {:?}", e))?;

    let signature = sk.sign(digest, BLS_SIGNING_DST, &[]);
    bls_g2_signature_bytes(&signature)
}

/// Convert blst G2 signature to 256-byte EIP-2537 format.
/// blst serialize: 192 bytes (4 x 48-byte Fp elements)
/// EIP-2537: 256 bytes (4 x 64-byte padded Fp elements)
pub fn bls_g2_signature_bytes(
    signature: &blst::min_pk::Signature,
) -> anyhow::Result<[u8; 256]> {
    let uncompressed = signature.serialize(); // 192 bytes
    let mut eip2537 = [0u8; 256];
    for i in 0..4 {
        // Each Fp: 16 zero bytes + 48 data bytes = 64 bytes
        let src_offset = i * 48;
        let dst_offset = i * 64 + 16;
        eip2537[dst_offset..dst_offset + 48]
            .copy_from_slice(&uncompressed[src_offset..src_offset + 48]);
    }
    Ok(eip2537)
}
```

### WavsSigner::sign() BLS Arm
```rust
// Source: packages/types/src/signing/signer.rs (to be modified)

#[cfg(feature = "bls")]
WavsCryptoSigner::Bls12381(ref bls_key) => {
    let hash = match kind.algorithm {
        SignatureAlgorithm::Bls12381 => self.unprefixed_hash()?,
        SignatureAlgorithm::Secp256k1 => {
            anyhow::bail!("Cannot sign secp256k1 with a BLS key")
        }
    };

    let digest: [u8; 32] = hash.into();
    let key = bls_key.clone();
    let kind = kind.clone();

    let (g2_sig, g1_pub) = tokio::task::spawn_blocking(move || {
        let g2 = utils::bls_signing::bls_sign_digest(&key, &digest)?;
        let g1 = utils::bls_signing::bls_g1_pubkey_bytes(&key)?;
        Ok::<_, anyhow::Error>((g2, g1))
    })
    .await
    .map_err(|e| anyhow::anyhow!("BLS signing task failed: {e}"))??;

    Ok(WavsSignature::Bls12381 {
        g2_signature: g2_sig.to_vec(),
        g1_pubkey: g1_pub.to_vec(),
        kind,
    })
}
```

### Dispatcher Algorithm Detection
```rust
// Source: packages/wavs/src/dispatcher.rs (to be modified)

fn add_service_to_managers(
    service: &Service,
    triggers: &TriggerManager,
    submissions: &SubmissionManager,
    aggregator_tx: &crossbeam::channel::Sender<AggregatorCommand>,
    hd_index: Option<u32>,
) -> Result<(), DispatcherError> {
    // Determine algorithm from the first workflow's submit configuration
    let algorithm = service
        .workflows
        .values()
        .find_map(|w| match &w.submit {
            Submit::Aggregator { signature_kind, .. } => Some(signature_kind.algorithm.clone()),
            Submit::None => None,
        })
        .unwrap_or(SignatureAlgorithm::Secp256k1); // default for backward compat

    if let Err(err) = submissions.add_service_key(service.id(), hd_index, algorithm) {
        // ... error handling unchanged ...
    }
    // ... rest unchanged ...
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| commonware Signer trait for BLS | Direct blst API with custom DST | Phase 6 | Required for contract compatibility; commonware's POP DST does not match contract |
| `unimplemented!()` BLS arms | Actual BLS signing pipeline | Phase 6 | Operators can now sign with BLS keys |
| `bls` feature opt-in | `bls` feature default-on | Phase 6 | All builds include BLS support without explicit `--features bls` |

**Key version facts (verified from Cargo.lock):**
- blst: 0.3.16 (already in Cargo.lock as transitive dep via commonware-cryptography)
- commonware-cryptography: 2026.3.0 (workspace dep)
- commonware-codec: 2026.3.0 (needed for PrivateKey Encode)

## Open Questions

1. **PrivateKey Encode byte order**
   - What we know: commonware's `PrivateKey` implements `Write` (from commonware-codec) which calls `self.raw.expose(|raw| raw.write(buf))`. The raw field is `Secret<[u8; 32]>`. blst's `SecretKey::from_bytes` expects big-endian scalar bytes.
   - What's unclear: Whether commonware's raw bytes are in the same byte order as blst expects. The `PrivateKey::read_cfg` calls `Private::decode(raw)` which suggests the bytes are in the format blst uses.
   - Recommendation: Add a unit test that round-trips: derive key with `bls_private_key_from_mnemonic`, extract bytes via Encode, construct `blst::min_pk::SecretKey`, and verify the public key matches. This test must pass before signing can be correct.

2. **SignerResponse BLS variant**
   - What we know: `SignerResponse` only has `Secp256k1 { hd_index, evm_address }`. The `get_service_signer` function has `unimplemented!()` for BLS.
   - What's unclear: Whether Phase 6 should add a BLS variant or just handle the error gracefully.
   - Recommendation: Add a minimal `Bls12381 { hd_index: u32, g1_pubkey_hex: String }` variant. The HTTP endpoint `/services/signer` already exists and will be called during MCP flows. A graceful response is better than a panic. If this seems too much scope, at minimum replace `unimplemented!()` with an error return.

3. **commonware-codec dependency in wavs-types**
   - What we know: The BLS signing code in `WavsSigner::sign()` lives in `packages/types/src/signing/signer.rs`. It needs to extract raw bytes from `PrivateKey` using the Encode trait.
   - What's unclear: Whether `commonware-codec` should be added as a dependency of `wavs-types`, or whether the BLS signing utility functions should live entirely in `packages/utils` (which already has blst as a dependency).
   - Recommendation: Keep all blst/signing logic in `packages/utils/src/bls_signing.rs`. The `WavsSigner::sign()` BLS arm should call a function from `utils` rather than importing commonware-codec into wavs-types. This keeps the crypto dependency boundary clean.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo workspace, per-crate |
| Quick run command | `cargo test -p utils -- bls && cargo test -p wavs-types --features full -- bls` |
| Full suite command | `cargo test -p utils && cargo test -p wavs-types --features full` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SIGN-01 | BLS sign digest produces 256-byte G2 signature | unit | `cargo test -p utils -- bls_signing::tests::sign_digest -x` | Wave 0 |
| SIGN-01 | G2 signature has correct EIP-2537 padding | unit | `cargo test -p utils -- bls_signing::tests::g2_signature_eip2537 -x` | Wave 0 |
| SIGN-01 | PrivateKey bytes roundtrip through blst SecretKey | unit | `cargo test -p utils -- bls_signing::tests::private_key_roundtrip -x` | Wave 0 |
| SIGN-02 | WavsSignature::Bls12381 serde roundtrip | unit | `cargo test -p wavs-types --features full -- signing::tests::wavs_signature_bls12381_serde -x` | Exists |
| SIGN-03 | Secp256k1 signing unchanged (regression) | unit | `cargo test -p wavs-types --features full -- signing::tests::wavs_signature_secp256k1_serde -x` | Exists |

### Sampling Rate
- **Per task commit:** `cargo test -p utils -- bls_signing && cargo test -p wavs-types --features full -- signing`
- **Per wave merge:** `cargo test -p utils && cargo test -p wavs-types --features full && cargo build`
- **Phase gate:** Full workspace build + all unit tests green before verify

### Wave 0 Gaps
- [ ] `packages/utils/src/bls_signing.rs` -- add `bls_sign_digest()` tests: roundtrip key extraction, 256-byte signature output, EIP-2537 padding correctness
- [ ] `packages/utils/src/bls_signing.rs` -- add `bls_g2_signature_bytes()` test: 192->256 byte padding
- [ ] Verify `commonware_codec::Encode` on `PrivateKey` produces bytes compatible with `blst::min_pk::SecretKey::from_bytes()` -- critical correctness test

## Sources

### Primary (HIGH confidence)
- `commonware-cryptography-2026.3.0` source code in cargo registry -- scheme.rs (Signer trait, sign method), group.rs (DST constants G2_MESSAGE = `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_`), ops/mod.rs (sign_message uses union_unique)
- `blst-0.3.16` source code in cargo registry -- lib.rs (SecretKey::sign, Signature::serialize)
- `contracts/poa-middleware/contracts/src/bls/libs/HashToCurve.sol` line 20 -- DST = `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`
- `contracts/poa-middleware/contracts/src/bls/POAStakeRegistry.sol` lines 240-241 -- `digest = keccak256(abi.encode(envelope))` then `hashToCurveG2(abi.encodePacked(digest))`
- `contracts/poa-middleware/contracts/src/bls/libs/BLS12381.sol` lines 21-22 -- `G1_POINT_SIZE = 128, G2_POINT_SIZE = 256`

### Secondary (MEDIUM confidence)
- `commonware-utils-2026.3.0` source code -- `union_unique()` function prepends varint-encoded namespace length (verified in source)
- Existing codebase patterns for `spawn_blocking` usage (aggregator/queue.rs, http/state.rs)

### Tertiary (LOW confidence)
- None -- all findings verified from source code

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in Cargo.lock, APIs verified from source
- Architecture: HIGH - DST mismatch identified and solution verified against contract source
- Pitfalls: HIGH - all identified from direct source code analysis, not assumptions
- Validation: MEDIUM - test structure proposed but exact byte-level correctness of G2 padding needs empirical verification

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable -- blst and commonware APIs unlikely to change within 30 days)
