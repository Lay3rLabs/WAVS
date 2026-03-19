# Phase 5: BLS Types and Key Derivation - Research

**Researched:** 2026-03-19
**Domain:** BLS12-381 cryptographic types, ABI bindings, deterministic key derivation
**Confidence:** HIGH

## Summary

Phase 5 introduces foundational BLS12-381 types into the WAVS codebase so that downstream phases (signing in Phase 6, aggregation in Phase 7) have a stable foundation. The work spans five areas: (1) adding a `Bls12381` variant to the `SignatureAlgorithm` enum in both Rust and WIT, (2) creating a unified `SignatureData` enum that wraps both secp256k1 and BLS inner types generated from their respective Solidity ABIs, (3) copying BLS poa-middleware contract ABIs into `packages/types` and generating Alloy bindings, (4) implementing deterministic BLS key derivation from a signing mnemonic using HKDF-SHA256 + ChaCha20Rng + commonware-cryptography, and (5) converting compressed 48-byte G1 public keys to 128-byte EIP-2537 uncompressed format.

All libraries are already present in the dependency tree. `commonware-cryptography 2026.3.0` provides `bls12381::PrivateKey::random(rng)` for key generation. The `blst 0.3.16` crate (transitive via commonware) provides low-level FFI for G1 point decompression. `hkdf 0.12.4` is already in `Cargo.lock` as a transitive dependency. The BLS ABI JSON files exist at `contracts/poa-middleware/contracts/out/bls/`. The `rand_chacha 0.3` pin (not 0.9) is critical for trait compatibility with `rand_core 0.6`.

**Primary recommendation:** Follow the established patterns exactly -- replicate `alloy_sol_macro::sol!()` binding pattern from `not_rpc.rs`/`rpc.rs` for BLS ABIs, replicate ed25519 key derivation pattern from `p2p.rs` with HKDF-SHA256 for HD index incorporation, and wrap existing secp256k1 code paths in enum variants with no behavioral changes.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **SignatureData**: Unified enum with `Secp256k1(inner)` and `Bls12381(inner)` variants, both inner types Alloy-generated from respective ABIs. Full migration of all call sites in Phase 5.
- **WavsSignature**: Convert from struct to enum with `Secp256k1 { data }` and `Bls12381 { g2_signature, g1_pubkey }` variants.
- **WavsCryptoSigner**: New enum with `Secp256k1(PrivateKeySigner)` and `Bls12381(commonware_cryptography::bls12381::PrivateKey)` variants. BLS arm stubs to `unimplemented!()`.
- **BLS ABI Bindings**: Copy 3 JSON files from `contracts/poa-middleware/contracts/out/bls/` into `packages/types/src/contracts/solidity/abi/bls/`. New `bls.rs` module for bindings.
- **WIT Interface**: Add `bls12381` variant to `signature-algorithm` in both `wit-definitions/types/wit/service.wit` and `wit-definitions/aggregator/wit/deps/wavs-types-2.7.0/package.wit`.
- **BLS Key Derivation**: New `packages/utils/src/bls_signing.rs` with `bls_private_key_from_mnemonic(mnemonic, hd_index)` using HKDF-SHA256 + ChaCha20Rng + `bls12381::PrivateKey::random(rng)`.
- **G1 Pubkey Helper**: `bls_g1_pubkey_bytes(pk) -> [u8; 128]` converting 48-byte ZCash compressed to 128-byte EIP-2537 uncompressed.
- **Tests**: Inline `#[cfg(test)]` in `bls_signing.rs` (determinism, G1 pubkey) and `packages/types` (SignatureData serde, BLS bindings compile).
- **BLS arm stubs**: `unimplemented!("BLS signing implemented in Phase 6")` for `WavsCryptoSigner` and `WavsSignature` BLS arms.

### Claude's Discretion
- Exact HKDF domain separation label (if any)
- How to extract G1 uncompressed point coordinates from commonware's type (may need blst FFI)
- Exact import structure within `packages/types/src/solidity_types/bls.rs`
- Whether to feature-gate BLS bindings under a `bls` feature flag in packages/types (or always-on)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TYPES-01 | `SignatureAlgorithm::Bls12381` variant added to Rust enum and WIT interface | WIT files identified (2 locations), Rust enum in `service.rs` line 558. Serde `rename_all = "snake_case"` means variant serializes as `bls12381`. |
| TYPES-02 | BLS submission carries G2 aggregate signature + sorted G1 signer pubkeys + reference block | BLS `IWavsServiceHandler.json` ABI confirmed: `signerPubkeys: bytes[]`, `aggregateSignature: bytes`, `referenceBlock: uint32`. Alloy will generate matching Rust struct. |
| TYPES-03 | poa-middleware BLS contract ABIs imported into `packages/types` | Three ABI JSON files confirmed at `contracts/poa-middleware/contracts/out/bls/` (6KB + 36KB + 17KB). `alloy_sol_macro::sol!()` pattern from `not_rpc.rs` provides exact template. |
| KEYS-01 | BLS private key derived deterministically from signing mnemonic per service (HD index) | `commonware_cryptography::bls12381::PrivateKey::random(rng: impl CryptoRngCore)` confirmed. Ed25519 derivation pattern in `p2p.rs` (lines 1368-1383) provides template. HKDF-SHA256 adds HD index differentiation. |
| KEYS-02 | BLS public key (G1 point, 128 bytes) derivable from private key for operator registration | PublicKey is 48-byte ZCash compressed G1. `blst::blst_p1_uncompress` + `blst_p1_affine_serialize` gives 96-byte uncompressed (x\|\|y). Pad each 48-byte coordinate to 64 bytes for 128-byte EIP-2537 format. Matches `BLS12381.G1_POINT_SIZE = 128` in contracts. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| commonware-cryptography | 2026.3.0 | BLS12-381 key generation and public key derivation | Already in packages/wavs, provides `bls12381::PrivateKey::random(rng)` and `Signer::public_key()` |
| blst | 0.3.16 | Low-level BLS12-381 FFI for G1 point decompression | Transitive dep via commonware, provides `blst_p1_uncompress` and `blst_p1_affine_serialize` for EIP-2537 conversion |
| hkdf | 0.12.4 | HKDF-SHA256 for incorporating HD index into BLS key seed | Already in Cargo.lock as transitive dep, standard KDF for deriving multiple keys from single seed |
| sha2 | 0.10.9 | SHA-256 hash function (required by hkdf) | Already workspace dependency |
| rand_chacha | 0.3 | Deterministic PRNG for key generation | Already in packages/wavs, MUST be 0.3 (not 0.9) for rand_core 0.6 compatibility with commonware |
| bip39 | 2.2.0 | BIP-39 mnemonic parsing and seed derivation | Already workspace dependency |
| alloy-sol-macro | workspace | Generate Rust types from Solidity ABI JSON | Already used for secp256k1 bindings in packages/types |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rand_core | 0.6 | CryptoRngCore trait required by commonware | Implicit via commonware-cryptography, provides the trait bounds for `PrivateKey::random()` |
| commonware-math | 2026.3.0 | `algebra::Random` trait for `PrivateKey::random()` | Already in packages/wavs, trait import needed for calling `random()` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| HKDF-SHA256 for HD index | Raw seed slicing (like ed25519 pattern) | HKDF is cryptographically proper for deriving multiple keys from one seed; raw slicing only works for single key |
| blst FFI for G1 decompression | Pure Rust BLS library | blst is already a dep, well-tested, and has the exact FFI functions needed |

**Installation (new deps for packages/utils):**
```toml
# In packages/utils/Cargo.toml [dependencies]
commonware-cryptography = "2026.3.0"
commonware-math = "2026.3.0"
rand_chacha = "0.3"
rand_core = "0.6"
bip39 = { workspace = true }
sha2 = { workspace = true }
hkdf = "0.12"
blst = "0.3.16"
```

## Architecture Patterns

### Recommended Project Structure
```
packages/types/src/
  solidity_types/
    mod.rs          # cfg_if selecting rpc.rs or not_rpc.rs (existing)
    rpc.rs          # secp256k1 bindings with #[sol(rpc)] (existing)
    not_rpc.rs      # secp256k1 bindings without rpc (existing)
    bls.rs          # NEW: BLS bindings (both rpc and not_rpc variants via cfg_if)
  contracts/solidity/abi/
    IWavsServiceHandler.sol/   # existing secp256k1
    IWavsServiceManager.sol/   # existing secp256k1
    bls/                       # NEW directory
      IWavsServiceHandler.json # BLS variant
      IPOAStakeRegistry.json   # BLS stake registry
      IWavsServiceManager.json # BLS service manager
  service.rs         # MODIFY: add Bls12381 variant to SignatureAlgorithm
  signing.rs         # MODIFY: SignatureData becomes enum, WavsSignature becomes enum
  signing/signer.rs  # MODIFY: WavsSigner trait updated, WavsCryptoSigner enum added

packages/utils/src/
  bls_signing.rs     # NEW: bls_private_key_from_mnemonic, bls_g1_pubkey_bytes

wit-definitions/types/wit/
  service.wit        # MODIFY: add bls12381 to signature-algorithm variant

wit-definitions/aggregator/wit/deps/wavs-types-2.7.0/
  package.wit        # MODIFY: add bls12381 to signature-algorithm variant
```

### Pattern 1: Unified SignatureData Enum
**What:** Replace the current Alloy-generated `SignatureData` struct (secp256k1-only) with a Rust enum that wraps both variants.
**When to use:** Any code that currently references `SignatureData` or `ServiceManagerSignatureData`.
**Example:**
```rust
// In packages/types/src/signing.rs (or new dedicated module)
// Source: CONTEXT.md locked decision

pub enum SignatureData {
    Secp256k1(secp256k1_binding::IWavsServiceHandler::SignatureData),
    Bls12381(bls_binding::IWavsServiceHandler::SignatureData),
}

// Migration: all current uses of SignatureData become SignatureData::Secp256k1(inner)
// The From<SignatureData> for ServiceManagerSignatureData impl must be updated
```

### Pattern 2: BLS ABI Bindings (replicating existing pattern)
**What:** Use `alloy_sol_macro::sol!()` to generate Rust types from BLS contract ABIs.
**When to use:** For BLS contract interaction types.
**Example:**
```rust
// In packages/types/src/solidity_types/bls.rs
// Source: packages/types/src/solidity_types/not_rpc.rs pattern

mod bls_service_handler {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IWavsServiceHandler,
        "./src/contracts/solidity/abi/bls/IWavsServiceHandler.json"
    );
}

mod bls_stake_registry {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IPOAStakeRegistry,
        "./src/contracts/solidity/abi/bls/IPOAStakeRegistry.json"
    );
}

mod bls_service_manager {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IWavsServiceManager,
        "./src/contracts/solidity/abi/bls/IWavsServiceManager.json"
    );
}

// Re-export with namespaced paths
pub use bls_service_handler::IWavsServiceHandler as BlsServiceHandler;
pub use bls_stake_registry::IPOAStakeRegistry as BlsStakeRegistry;
pub use bls_service_manager::IWavsServiceManager as BlsServiceManager;
```

### Pattern 3: BLS Key Derivation (extending ed25519 pattern)
**What:** Deterministically derive a BLS private key from a BIP-39 mnemonic and HD index.
**When to use:** When a WAVS operator needs a BLS signing key for a specific service.
**Example:**
```rust
// In packages/utils/src/bls_signing.rs
// Source: ed25519_signer_from_mnemonic() in packages/wavs/src/subsystems/aggregator/p2p.rs

use commonware_cryptography::bls12381;
use commonware_math::algebra::Random;
use hkdf::Hkdf;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use sha2::Sha256;

pub fn bls_private_key_from_mnemonic(
    mnemonic: &str,
    hd_index: u32,
) -> anyhow::Result<bls12381::PrivateKey> {
    // Guard: reject raw private keys
    if mnemonic.starts_with("0x") {
        anyhow::bail!("BLS key derivation requires a mnemonic, not a raw key");
    }

    // Parse BIP-39 mnemonic
    let mnemonic = bip39::Mnemonic::parse(mnemonic)
        .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))?;

    // Derive 64-byte BIP-39 seed (empty passphrase)
    let seed = mnemonic.to_seed("");

    // HKDF-SHA256: incorporate HD index into the key material
    let hk = Hkdf::<Sha256>::new(None, &seed);
    let mut rng_seed = [0u8; 32];
    hk.expand(&hd_index.to_le_bytes(), &mut rng_seed)
        .map_err(|e| anyhow::anyhow!("HKDF expand failed: {}", e))?;

    // Deterministic RNG seeded from HKDF output
    let mut rng = ChaCha20Rng::from_seed(rng_seed);

    // Generate BLS private key
    Ok(bls12381::PrivateKey::random(&mut rng))
}
```

### Pattern 4: G1 Pubkey EIP-2537 Conversion
**What:** Convert 48-byte ZCash compressed G1 point to 128-byte EIP-2537 uncompressed format.
**When to use:** When preparing a BLS public key for on-chain registration via `updateOperatorSigningKey`.
**Example:**
```rust
// In packages/utils/src/bls_signing.rs
// Source: BLS12381.sol G1_POINT_SIZE = 128, blst FFI

use commonware_cryptography::{bls12381, Signer as _};

pub fn bls_g1_pubkey_bytes(
    private_key: &bls12381::PrivateKey,
) -> anyhow::Result<[u8; 128]> {
    let pubkey = private_key.public_key();
    let compressed: &[u8] = pubkey.as_ref(); // 48-byte ZCash compressed

    // Decompress to affine point via blst FFI
    let mut affine = blst::blst_p1_affine::default();
    let result = unsafe {
        blst::blst_p1_uncompress(&mut affine, compressed.as_ptr())
    };
    if result != blst::BLST_ERROR::BLST_SUCCESS {
        anyhow::bail!("Failed to uncompress G1 point: {:?}", result);
    }

    // Serialize to 96-byte uncompressed (x || y, each 48 bytes big-endian)
    let mut uncompressed = [0u8; 96];
    unsafe {
        blst::blst_p1_affine_serialize(uncompressed.as_mut_ptr(), &affine);
    }

    // Pad each 48-byte coordinate to 64 bytes (EIP-2537 format)
    let mut eip2537 = [0u8; 128];
    // x: 16 zero bytes + 48-byte x
    eip2537[16..64].copy_from_slice(&uncompressed[0..48]);
    // y: 16 zero bytes + 48-byte y
    eip2537[80..128].copy_from_slice(&uncompressed[48..96]);

    Ok(eip2537)
}
```

### Anti-Patterns to Avoid
- **Breaking secp256k1 path behavior**: All existing secp256k1 code paths must remain functionally identical -- only wrapped in `SignatureData::Secp256k1(inner)`. Do not change signing logic, address recovery, or ABI encoding.
- **Using rand_chacha 0.9**: This version depends on `rand_core 0.9` which is trait-incompatible with commonware's `rand_core 0.6`. Must pin to `rand_chacha = "0.3"`.
- **Implementing BLS signing logic in Phase 5**: BLS arms of `WavsCryptoSigner::sign()` and `WavsSignature` methods must be `unimplemented!()` stubs. Signing logic belongs in Phase 6.
- **Duplicating SignatureData instead of using an enum**: The secp256k1 and BLS `SignatureData` have different fields (`signers: address[]` vs `signerPubkeys: bytes[]`). They cannot share a single struct -- an enum is required.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BLS key generation | Custom scalar sampling from blst | `commonware_cryptography::bls12381::PrivateKey::random(rng)` | Handles subgroup checking, zeroization, proper scalar range |
| KDF for HD index | Manual hash-based key derivation | `hkdf::Hkdf::<Sha256>` | Cryptographically proven KDF, handles edge cases (empty salt, short IKM) |
| G1 point decompression | Custom curve math | `blst::blst_p1_uncompress` | Constant-time, validates point-on-curve, handles compressed format correctly |
| BIP-39 seed derivation | Custom PBKDF2 | `bip39::Mnemonic::to_seed("")` | Handles PBKDF2 with 2048 rounds, proper Unicode normalization |
| Solidity ABI type generation | Manual struct definitions | `alloy_sol_macro::sol!()` | Generates correct ABI encoding/decoding, keeps Rust types in sync with Solidity |

**Key insight:** Every cryptographic operation in this phase has a well-tested library implementation already in the dependency tree. The novel code is limited to glue: HKDF info field construction, coordinate padding for EIP-2537, and enum wrapping.

## Common Pitfalls

### Pitfall 1: rand_chacha Version Mismatch
**What goes wrong:** `rand_chacha 0.9` pulls in `rand_core 0.9`, breaking the `CryptoRngCore` trait bound expected by `commonware_cryptography::bls12381::PrivateKey::random()`.
**Why it happens:** Cargo may resolve to 0.9 if the version specifier is `"*"` or `">=0.3"`.
**How to avoid:** Pin exactly `rand_chacha = "0.3"` in packages/utils Cargo.toml. Verify with `cargo tree -p utils -i rand_chacha`.
**Warning signs:** Compilation error about `CryptoRngCore` trait not implemented for `ChaCha20Rng`.

### Pitfall 2: Alloy sol! Macro Module Name Collision
**What goes wrong:** Both secp256k1 and BLS ABIs define `IWavsServiceHandler` with different structs (`signers: address[]` vs `signerPubkeys: bytes[]`). Using the same module names causes compilation errors.
**Why it happens:** `alloy_sol_macro::sol!()` generates types at module scope. Two modules with the same interface name conflict.
**How to avoid:** Put BLS bindings in a separate module (e.g., `bls_service_handler`) in a separate file (`bls.rs`). Re-export with disambiguating names.
**Warning signs:** "conflicting implementations" or "duplicate definitions" errors.

### Pitfall 3: G1 Coordinate Padding Direction
**What goes wrong:** EIP-2537 expects coordinates left-padded with zeros to 64 bytes. Padding on the right produces an invalid point that fails on-chain verification.
**Why it happens:** Each BLS12-381 Fp element is 48 bytes big-endian. EIP-2537 uses 64-byte fields (512 bits for a 381-bit field). The padding must be leading zeros.
**How to avoid:** `eip2537[16..64].copy_from_slice(&x_48_bytes)` -- 16 zero bytes prefix, then 48 data bytes.
**Warning signs:** On-chain pairing check fails with `PrecompileCallFailed`. G1 point size is 128 but data appears shifted.

### Pitfall 4: Incomplete SignatureData Migration
**What goes wrong:** A call site still uses `SignatureData { signers, signatures, referenceBlock }` (struct field access) instead of `SignatureData::Secp256k1(inner)` (enum variant).
**Why it happens:** The migration touches multiple files across packages/types, packages/wavs, and packages/utils.
**How to avoid:** Use `cargo build` after each file change. The compiler will catch every unmigrated usage because the type shape changes from struct to enum.
**Warning signs:** Compilation errors about missing struct fields or unexpected enum variant.

### Pitfall 5: WIT File Synchronization
**What goes wrong:** Adding `bls12381` to `service.wit` but forgetting the pinned copy in `aggregator/wit/deps/wavs-types-2.7.0/package.wit`. Or vice versa.
**Why it happens:** The WIT definitions exist in two places that must stay in sync.
**How to avoid:** Update both files in the same task. Verify with `just wasi-build-native` (which processes WIT definitions).
**Warning signs:** WASI component build fails with "variant mismatch" or "unknown variant" errors.

### Pitfall 6: blst FFI Safety
**What goes wrong:** Passing an invalid compressed point to `blst_p1_uncompress` causes undefined behavior or panics.
**Why it happens:** The FFI functions expect valid BLS12-381 points. Garbage input may pass the pointer check but produce wrong results.
**How to avoid:** Only call `blst_p1_uncompress` with bytes from `commonware_cryptography::bls12381::PublicKey::as_ref()` which is guaranteed to be a valid compressed G1 point. Always check the `BLST_ERROR` return value.
**Warning signs:** Non-deterministic test failures, SIGSEGV in blst functions.

## Code Examples

### BLS ABI SignatureData Fields (verified from JSON)
```json
// Source: contracts/poa-middleware/contracts/out/bls/IWavsServiceHandler.sol/IWavsServiceHandler.json
{
  "name": "signatureData",
  "components": [
    {"name": "signerPubkeys", "type": "bytes[]"},
    {"name": "aggregateSignature", "type": "bytes"},
    {"name": "referenceBlock", "type": "uint32"}
  ]
}
```

### Secp256k1 ABI SignatureData Fields (for comparison)
```json
// Source: packages/types/src/contracts/solidity/abi/IWavsServiceHandler.sol/IWavsServiceHandler.json
{
  "name": "signatureData",
  "components": [
    {"name": "signers", "type": "address[]"},
    {"name": "signatures", "type": "bytes[]"},
    {"name": "referenceBlock", "type": "uint32"}
  ]
}
```

### Ed25519 Key Derivation Pattern (template for BLS)
```rust
// Source: packages/wavs/src/subsystems/aggregator/p2p.rs lines 1368-1383
pub fn ed25519_signer_from_mnemonic(mnemonic: &str) -> Result<ed25519::PrivateKey, AggregatorError> {
    let mnemonic = bip39::Mnemonic::parse(mnemonic)
        .map_err(|e| AggregatorError::P2p(format!("Invalid mnemonic: {}", e)))?;
    let seed = mnemonic.to_seed("");
    let rng_seed: [u8; 32] = seed[..32].try_into()
        .map_err(|_| AggregatorError::P2p("BIP-39 seed too short".into()))?;
    use rand_chacha::rand_core::SeedableRng;
    let mut rng = ChaCha20Rng::from_seed(rng_seed);
    Ok(ed25519::PrivateKey::random(&mut rng))
}
// NOTE: BLS version adds HKDF step to incorporate hd_index
```

### Existing sol! Binding Pattern
```rust
// Source: packages/types/src/solidity_types/not_rpc.rs
mod service_handler {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IWavsServiceHandler,
        "./src/contracts/solidity/abi/IWavsServiceHandler.sol/IWavsServiceHandler.json"
    );
}
pub use service_handler::{
    IWavsServiceHandler, IWavsServiceHandler::Envelope, IWavsServiceHandler::SignatureData,
};
```

### EIP-2537 G1 Point Format (from BLS12381.sol)
```solidity
// Source: contracts/poa-middleware/contracts/src/bls/libs/BLS12381.sol
uint256 internal constant G1_POINT_SIZE = 128;  // 2 x 64-byte Fp elements
uint256 internal constant G2_POINT_SIZE = 256;  // 2 x 128-byte Fp2 elements

// G1 generator coordinates (128 hex chars each = 64 bytes each):
// x = 0x0000000000000000000000000000000017f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb
// Format: 16 zero bytes (padding) + 48-byte big-endian coordinate
```

### Commonware BLS12381 Key Sizes
```rust
// Source: commonware-cryptography-2026.3.0/src/bls12381/primitives/group.rs
pub const G1_ELEMENT_BYTE_LENGTH: usize = 48;   // compressed (ZCash format)
pub const G2_ELEMENT_BYTE_LENGTH: usize = 96;   // compressed (ZCash format)
pub const PRIVATE_KEY_LENGTH: usize = 32;        // scalar field element

// PrivateKey::SIZE = 32 bytes
// PublicKey::SIZE  = 48 bytes (compressed G1 via blst_p1_compress)
// Signature::SIZE  = 96 bytes (compressed G2 via blst_p2_compress)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single `SignatureData` struct from secp256k1 ABI | Enum wrapping both secp256k1 and BLS inner types | Phase 5 | All call sites must use `SignatureData::Secp256k1(inner)` |
| `WavsSignature` struct with `data: Vec<u8>` | Enum with algorithm-specific variants | Phase 5 | BLS signatures carry G1 pubkey alongside G2 sig |
| `WavsSigner::sign()` takes `&PrivateKeySigner` | Takes `&WavsCryptoSigner` enum | Phase 5 | Enables algorithm dispatch in Phase 6 |
| No BLS types in codebase | Full type foundation | Phase 5 | Phases 6-8 can focus on logic, not types |

**Deprecated/outdated:**
- Direct `SignatureData { signers, signatures, referenceBlock }` struct field access -- must use enum variant after Phase 5

## Open Questions

1. **HKDF Domain Separation Label**
   - What we know: HKDF takes an optional `info` parameter. We're using `hd_index.to_le_bytes()` as info.
   - What's unclear: Whether to also include a domain separation label like `b"WAVS-BLS-KEY-v1"` prepended to the info.
   - Recommendation: Include a domain label for safety: `[b"WAVS-BLS-KEY-v1", &hd_index.to_le_bytes()]` concatenated as info. This prevents accidental collision with other HKDF usages of the same seed. Claude's discretion per CONTEXT.md.

2. **Feature-gating BLS Bindings**
   - What we know: The BLS ABI bindings will be compiled unconditionally alongside secp256k1 bindings. Feature flags exist in packages/types for other optional deps (e.g., `signer`, `solidity-rpc`).
   - What's unclear: Whether BLS bindings should be behind a `bls` feature flag.
   - Recommendation: Always-on (no feature gate). BLS is not an optional feature -- it's part of the core type system. Feature gates add complexity for no benefit since the bindings are just type definitions with no runtime cost. Claude's discretion per CONTEXT.md.

3. **blst G1 Uncompressed Extraction Method**
   - What we know: `blst_p1_uncompress` converts 48-byte compressed to `blst_p1_affine`, then `blst_p1_affine_serialize` writes 96 bytes (x || y, each 48 bytes).
   - What's unclear: Whether to use `blst_p1_affine_serialize` (96 bytes, then manually pad) or directly access `affine.x.l` and `affine.y.l` fields.
   - Recommendation: Use `blst_p1_affine_serialize` which writes canonical big-endian bytes, then pad each 48-byte half to 64 bytes. This is safer than manually interpreting internal limb representation. Claude's discretion per CONTEXT.md.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + cargo test |
| Config file | Workspace Cargo.toml |
| Quick run command | `cargo test -p utils --lib bls_signing` |
| Full suite command | `cargo test -p utils -p wavs-types` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TYPES-01 | `SignatureAlgorithm::Bls12381` compiles and serializes | unit | `cargo test -p wavs-types --lib` | Wave 0 |
| TYPES-02 | BLS `SignatureData` enum variant accessible with correct fields | unit | `cargo test -p wavs-types --lib` | Wave 0 |
| TYPES-03 | BLS Alloy bindings compile and types are accessible | unit | `cargo test -p wavs-types --lib` | Wave 0 |
| KEYS-01 | Deterministic BLS key from mnemonic + HD index | unit | `cargo test -p utils --lib bls_signing` | Wave 0 |
| KEYS-02 | 128-byte G1 pubkey derivable from private key | unit | `cargo test -p utils --lib bls_signing` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo build -p wavs-types -p utils` (type check) + `cargo test -p utils --lib bls_signing -p wavs-types --lib` (unit tests)
- **Per wave merge:** `cargo test -p utils -p wavs-types -p wavs` (full package test suite)
- **Phase gate:** `just lint && cargo test -p utils -p wavs-types -p wavs` -- full lint + test before /gsd:verify-work

### Wave 0 Gaps
- [ ] `packages/utils/src/bls_signing.rs` -- covers KEYS-01, KEYS-02 (new file)
- [ ] BLS binding compile test in packages/types -- covers TYPES-03 (inline test)
- [ ] SignatureData enum serde test in packages/types -- covers TYPES-02 (inline test)
- [ ] SignatureAlgorithm serde test including Bls12381 -- covers TYPES-01 (inline test)

*(All tests will be inline `#[cfg(test)]` modules per project convention)*

## Sources

### Primary (HIGH confidence)
- `commonware-cryptography-2026.3.0/src/bls12381/scheme.rs` -- PrivateKey, PublicKey, Signature types, Random trait impl, MinPk variant
- `commonware-cryptography-2026.3.0/src/bls12381/primitives/group.rs` -- G1/G2 element sizes (48/96 compressed), blst FFI usage patterns, `as_blst_p1_affine()`, `blst_p1_compress`
- `commonware-cryptography-2026.3.0/src/bls12381/primitives/variant.rs` -- MinPk variant: PublicKey=G1, Signature=G2
- `blst-0.3.16/blst/bindings/blst.h` -- `blst_p1_affine` struct layout, `blst_p1_uncompress`, `blst_p1_affine_serialize` (96 bytes output)
- `contracts/poa-middleware/contracts/out/bls/IWavsServiceHandler.sol/IWavsServiceHandler.json` -- BLS SignatureData ABI structure
- `contracts/poa-middleware/contracts/src/bls/libs/BLS12381.sol` -- G1_POINT_SIZE=128, G2_POINT_SIZE=256, EIP-2537 coordinate format
- `packages/types/src/solidity_types/not_rpc.rs` -- Exact `alloy_sol_macro::sol!()` binding pattern
- `packages/wavs/src/subsystems/aggregator/p2p.rs` (lines 1368-1383) -- `ed25519_signer_from_mnemonic()` derivation pattern
- `packages/types/src/service.rs` -- `SignatureAlgorithm`, `SignatureKind` current definitions
- `packages/types/src/signing/signer.rs` -- `WavsSigner`, `WavsSignature`, `signature_data()` current implementation

### Secondary (MEDIUM confidence)
- `hkdf 0.12.4` API -- `Hkdf::<Sha256>::new(salt, ikm)` and `expand(info, okm)` -- standard HKDF-SHA256 API, well-documented
- EIP-2537 format specification -- 64-byte Fp elements with leading zero padding for 381-bit field

### Tertiary (LOW confidence)
- None -- all findings verified from source code and local dependency tree

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies verified in Cargo.lock, versions confirmed, API surface inspected in source
- Architecture: HIGH -- patterns directly replicate existing codebase conventions (sol! macro, key derivation, WIT updates)
- Pitfalls: HIGH -- identified from actual code inspection (module naming, coordinate padding, version pins)
- Key derivation: HIGH -- HKDF + ChaCha20Rng + commonware PrivateKey::random() is well-established pattern

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable domain, no fast-moving dependencies)
