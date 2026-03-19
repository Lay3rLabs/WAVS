# Phase 5: BLS Types and Key Derivation - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Add BLS12-381 foundational types so Phase 6 (signing) and Phase 7 (aggregation) have a solid foundation to build on. Deliverables:

- `SignatureAlgorithm::Bls12381` variant in Rust enum and WIT interface
- Unified `SignatureData` enum replacing the existing secp256k1-only type
- `WavsSignature` and `WavsCryptoSigner` enums extended for BLS (BLS arms stub out to `unimplemented!()`)
- BLS poa-middleware contract ABI bindings in `packages/types`
- `bls_private_key_from_mnemonic(mnemonic, hd_index)` in `packages/utils`
- `bls_g1_pubkey_bytes(pk)` returning 128-byte EIP-2537 G1 pubkey

No signing logic (Phase 6), no aggregation (Phase 7), no E2E tests (Phase 8).

</domain>

<decisions>
## Implementation Decisions

### SignatureData — Unified enum, full migration

- Replace the existing `SignatureData` type (currently `IWavsServiceHandler::SignatureData` from secp256k1 ABI) with a new Rust enum:
  ```rust
  pub enum SignatureData {
      Secp256k1(secp256k1_binding::IWavsServiceHandler::SignatureData),
      Bls12381(bls_binding::IWavsServiceHandler::SignatureData),
  }
  ```
- Both inner types are **Alloy-generated** from their respective `IWavsServiceHandler.json` ABIs (secp256k1 and BLS)
- The BLS `SignatureData` struct from the BLS ABI has fields: `signerPubkeys: bytes[]`, `aggregateSignature: bytes`, `referenceBlock: uint32`
- BLS data is stored in **EIP-2537 format**: 128-byte G1 pubkeys, 256-byte G2 aggregate signature (uncompressed, precompile-ready)
- **Full migration in Phase 5**: all call sites in `packages/wavs`, `packages/utils`, etc. updated to `SignatureData::Secp256k1(inner)`
- Root re-export at `wavs_types::SignatureData` — import paths unchanged, only enum access changes

### WavsSignature — enum variant

- Convert from struct to enum:
  ```rust
  pub enum WavsSignature {
      Secp256k1 { data: Vec<u8> },
      Bls12381 { g2_signature: Vec<u8>, g1_pubkey: Vec<u8> },
  }
  ```
- BLS signature is not self-recovering — G1 pubkey must travel with the signature
- `signature_data()` on `WavsSignable` stays as the single entry point; dispatches by variant to produce `SignatureData::Secp256k1` or `SignatureData::Bls12381`

### WavsCryptoSigner — new enum, Phase 5 defines structure

- New enum:
  ```rust
  pub enum WavsCryptoSigner {
      Secp256k1(PrivateKeySigner),
      Bls12381(commonware_cryptography::bls12381::PrivateKey),
  }
  ```
- `WavsSigner::sign(signer: &WavsCryptoSigner, kind: SignatureKind)` signature updated to accept enum
- BLS arm: `unimplemented!("BLS signing implemented in Phase 6")`
- Secp256k1 path unchanged in behavior — just wrapped in the enum

### BLS ABI Bindings — copy into packages/types

- Copy 3 JSON files from `contracts/poa-middleware/contracts/out/bls/` into `packages/types/src/contracts/solidity/abi/bls/`:
  - `IWavsServiceHandler.json`
  - `IPOAStakeRegistry.json`
  - `IWavsServiceManager.json`
- New Alloy bindings file: `packages/types/src/solidity_types/bls.rs`
- Module naming must disambiguate from secp256k1 bindings (e.g., `bls_service_handler::IWavsServiceHandler`)

### WIT Interface — add bls12381 variant

- `wit-definitions/types/wit/service.wit`:
  ```wit
  variant signature-algorithm {
      secp256k1,
      bls12381,  // add this
  }
  ```
- `signature-prefix` unchanged — BLS uses `None` (no EIP-191 prefix, hash-to-curve uses its own DST)
- Update **all WIT definition copies** including `wit-definitions/aggregator/wit/deps/wavs-types-*/package.wit`

### BLS Key Derivation — new packages/utils/src/bls_signing.rs

- New file: `packages/utils/src/bls_signing.rs`
- Primary function:
  ```rust
  pub fn bls_private_key_from_mnemonic(
      mnemonic: &str,
      hd_index: u32,
  ) -> Result<commonware_cryptography::bls12381::PrivateKey>
  ```
- Algorithm:
  1. Reject if mnemonic starts with `0x` — error: "BLS key derivation requires a mnemonic, not a raw key"
  2. Parse BIP-39 mnemonic
  3. Derive 64-byte BIP-39 seed (empty passphrase)
  4. HKDF-SHA256(ikm=seed, info=hd_index.to_le_bytes()) → 32-byte RNG seed
  5. `ChaCha20Rng::from_seed(rng_seed)`
  6. `bls12381::PrivateKey::random(&mut rng)`
- Returns `commonware_cryptography::bls12381::PrivateKey` (consistent with `ed25519_signer_from_mnemonic` returning `ed25519::PrivateKey`)

- G1 pubkey helper:
  ```rust
  pub fn bls_g1_pubkey_bytes(
      private_key: &commonware_cryptography::bls12381::PrivateKey,
  ) -> [u8; 128]
  ```
  Derives G1 pubkey via commonware, converts from 48-byte ZCash compressed to 128-byte EIP-2537 uncompressed format (pad each 48-byte coordinate to 64 bytes with leading zeros)

- New Cargo.toml deps for `packages/utils`:
  - `commonware-cryptography = "2026.3.0"` (same version already in packages/wavs)
  - `hkdf` (for HKDF-SHA256)
  - `sha2` (if not already present)

### Tests — inline in bls_signing.rs

- `#[cfg(test)]` in `packages/utils/src/bls_signing.rs`:
  - **Determinism**: same mnemonic + HD index → same key bytes; different HD indices → different keys
  - **G1 pubkey**: 128-byte output, deterministic, correct length
- In `packages/types` tests:
  - `SignatureData` enum serde round-trip for both variants
  - BLS Alloy bindings compile and types are accessible

### Claude's Discretion

- Exact HKDF domain separation label (if any)
- How to handle blst G1 uncompressed point coordinate extraction from commonware's type (may need `unsafe` blst FFI or public API)
- Exact import structure within packages/types/src/solidity_types/bls.rs
- Whether to feature-gate the BLS bindings under a `bls` feature flag in packages/types (or always-on)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing type definitions to extend
- `packages/types/src/service.rs` — `SignatureAlgorithm`, `SignatureKind`, `SignaturePrefix` Rust enums (add `Bls12381` variant)
- `packages/types/src/signing.rs` — `WavsSignable` trait, current `SignatureData` re-export; base for enum migration
- `packages/types/src/signing/signer.rs` — `WavsSigner` trait, `WavsSignature` struct, `signature_data()` method; all become enum-based
- `packages/types/src/solidity_types/not_rpc.rs` — exact pattern for `alloy_sol_macro::sol!()` ABI bindings; replicate for BLS

### WIT interface files to update
- `wit-definitions/types/wit/service.wit` — `variant signature-algorithm { secp256k1 }` → add `bls12381`
- `wit-definitions/aggregator/wit/deps/wavs-types-2.7.0/package.wit` — pinned copy to update

### Key derivation patterns to follow
- `packages/wavs/src/subsystems/aggregator/p2p.rs` — `ed25519_signer_from_mnemonic()` (lines ~1368+): exact ChaCha20Rng seeding pattern to replicate for BLS
- `packages/utils/src/evm_client/signing.rs` — `make_signer(credentials, hd_index)` secp256k1 HD derivation and `0x` guard pattern

### poa-middleware BLS contracts (source of ABIs and size constants)
- `contracts/poa-middleware/contracts/src/bls/interfaces/IWavsServiceHandler.sol` — BLS `SignatureData` struct: `signerPubkeys: bytes[]`, `aggregateSignature: bytes`, `referenceBlock: uint32`
- `contracts/poa-middleware/contracts/src/bls/libs/BLS12381.sol` — `G1_POINT_SIZE = 128`, `G2_POINT_SIZE = 256`; EIP-2537 precompile format
- `contracts/poa-middleware/contracts/src/bls/POAStakeRegistry.sol` — `updateOperatorSigningKey(blsKey, blsSigProof)` takes 128-byte G1 key; `_checkSignatures` validates aggregate
- `contracts/poa-middleware/contracts/out/bls/IWavsServiceHandler.sol/IWavsServiceHandler.json` — ABI JSON to copy
- `contracts/poa-middleware/contracts/out/bls/IPOAStakeRegistry.sol/IPOAStakeRegistry.json` — ABI JSON to copy
- `contracts/poa-middleware/contracts/out/bls/IWavsServiceManager.sol/IWavsServiceManager.json` — ABI JSON to copy

### commonware BLS crypto
- `~/.cargo/registry/src/.../commonware-cryptography-2026.3.0/src/bls12381/mod.rs` — exports `PrivateKey`, `PublicKey`, `Signature`; BETA stability, no feature flag needed
- `~/.cargo/registry/src/.../commonware-cryptography-2026.3.0/src/bls12381/scheme.rs` — `PrivateKey::random(rng: impl CryptoRngCore)` — entry point for key generation

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `commonware_cryptography::bls12381::PrivateKey::random(rng)` — already a transitive dep via commonware-p2p, BETA stability (same as ed25519), available without feature flags
- `ChaCha20Rng` from `rand_chacha = "0.3"` — already in packages/wavs Cargo.toml, used in `ed25519_signer_from_mnemonic()` in p2p.rs; exact pattern to replicate for BLS
- `alloy_sol_macro::sol!()` in `packages/types/src/solidity_types/not_rpc.rs` — exact pattern to clone for BLS ABI bindings in new `bls.rs`
- `bip39::Mnemonic::parse(mnemonic)` — already imported in p2p.rs, same crate for BLS derivation

### Established Patterns
- Ed25519 key derivation: `bip39_seed[..32] → ChaCha20Rng::from_seed → ed25519::PrivateKey::random(rng)` — replicate but use HKDF with HD index
- Secp256k1 HD derivation: `MnemonicBuilder::<English>::default().phrase().index(hd_index).build()` in signing.rs — parallel exists for reference
- ABI bindings: module wrapping + `alloy_sol_macro::sol!(InterfaceName, "./path/to/abi.json")` + `pub use` re-exports
- SignatureKind already has `evm_default()` factory; consider adding `bls_default()` for Phase 5 completeness

### Integration Points
- `packages/wavs/src/subsystems/submission.rs` uses `WavsSigner::sign()` — call site needs `WavsCryptoSigner` enum update
- `packages/wavs/src/subsystems/aggregator/p2p.rs` uses `SignatureKind` and `WavsSignature` in `Submission` struct — both change shape
- `packages/types/src/signing.rs` re-exports `SignatureData` at crate root — this becomes the enum re-export
- `packages/types/src/contracts/cosmwasm/service_manager.rs` constructs `SignatureData` directly (line ~205) — secp256k1 path needs migration to enum
- `packages/wavs-mcp/src/chain_ops.rs` uses `EvmSigningClient` with HD index — separate from BLS derivation, not a migration target in Phase 5

</code_context>

<specifics>
## Specific Ideas

- Use HKDF-SHA256 for incorporating HD index into the BLS key seed — `HKDF(ikm=bip39_seed, info=hd_index.to_le_bytes())` → 32-byte ChaCha20Rng seed
- G1 pubkey EIP-2537 conversion: pad each 48-byte compressed coordinate to 64 bytes with leading zeros → 128 bytes total (verify against `BLS12381.G1_POINT_SIZE = 128` in poa-middleware)
- BLS arm of `WavsCryptoSigner` and `WavsSignature` in Phase 5 explicitly `unimplemented!()` so Phase 6 has clear stubs to fill in
- The `rand_chacha = "0.3"` version pin (not 0.9) is critical — commonware uses `rand_core 0.6` and the newer version has trait mismatches (see PROJECT.md Key Decisions table)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 05-bls-types-and-key-derivation*
*Context gathered: 2026-03-19*
