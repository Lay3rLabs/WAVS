# Phase 6: BLS Signing Pipeline - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

An operator configured for BLS can sign a submission envelope with its BLS key and propagate the signed submission (G2 signature + G1 pubkey) over P2P. Secp256k1 services continue working unchanged.

Deliverables:
- `WavsCryptoSigner::Bls12381` arm in `WavsSigner::sign()` — actual blst signing (replaces `unimplemented!()`)
- `add_service_key` reads `signature_kind.algorithm` from caller and creates `WavsCryptoSigner::Bls12381` for BLS services
- `Submission` propagated over P2P carries `WavsSignature::Bls12381 { g2_signature, g1_pubkey, kind }` for BLS services
- `bls` feature default-on in both `packages/types` and `packages/wavs`

No BLS aggregation (Phase 7), no E2E tests (Phase 8), no MCP tooling changes.

</domain>

<decisions>
## Implementation Decisions

### Algorithm detection

- `add_service_key` signature extended to accept `SignatureAlgorithm` (passed by caller)
- The dispatcher reads `service.signature_kind.algorithm` (from `Submit::Aggregator`) and passes it to `add_service_key`
- `add_service_key` creates `WavsCryptoSigner::Bls12381(bls_private_key_from_mnemonic(mnemonic, hd_index)?)` when `SignatureAlgorithm::Bls12381`
- Secp256k1 services: `WavsCryptoSigner::Secp256k1(make_signer(mnemonic, hd_index)?)` — unchanged

### Feature flags

- `bls` feature is **default-on** in `packages/types` Cargo.toml: add `bls` to `[features] default = [...]`
- `bls` feature is **default-on** in `packages/wavs` Cargo.toml: same
- All WAVS deployments support BLS without requiring `--features bls`

### Signing implementation

- BLS signing replaces `unimplemented!()` in `WavsCryptoSigner::Bls12381` arm of `WavsSigner::sign()`
- Signing input: `keccak256(abi_encode(envelope))` as 32 bytes — matches contract: `digest = keccak256(abi.encode(envelope))`
- blst call: `Sign::sign(&private_key, &digest_bytes, b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_")`
- blst is CPU-bound: wrap in `tokio::task::spawn_blocking`
- Returns `WavsSignature::Bls12381 { g2_signature: sig_bytes (256 bytes), g1_pubkey: pubkey_bytes (128 bytes), kind }`

### Secp256k1 path

- Zero behavioral changes to secp256k1 signing, signer creation, or P2P propagation
- `SIGN-03` satisfied by not touching any secp256k1 code paths

### Claude's Discretion

- SignerResponse BLS variant: not selected for discussion — defer to Phase 8 or handle as Claude sees fit (stub or forward-looking variant)
- Error behavior for `0x` raw key with BLS service: handle at `add_service_key` time with clear error message (consistent with existing `0x` guard in `bls_private_key_from_mnemonic`)
- Exact blst API call shape (Sign trait vs raw FFI) — use whatever is cleanest with `commonware-cryptography`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Signing pipeline files to modify
- `packages/wavs/src/subsystems/submission.rs` — `add_service_key()`, `get_service_signer()`, `sign_request()`; BLS arm stubs are `unimplemented!()`
- `packages/types/src/signing/signer.rs` — `WavsCryptoSigner` enum, `WavsSigner::sign()` trait; BLS arm is `unimplemented!("BLS signing implemented in Phase 6")`
- `packages/wavs/src/dispatcher.rs:1067` — `add_service_to_managers()` calls `add_service_key(service.id(), hd_index)`; extend to pass `signature_algorithm`

### Phase 5 BLS utilities (already implemented)
- `packages/utils/src/bls_signing.rs` — `bls_private_key_from_mnemonic(mnemonic, hd_index)` and `bls_g1_pubkey_bytes(pk)` (128-byte EIP-2537 G1 pubkey)
- `packages/types/src/signing.rs` — `WavsSignature::Bls12381 { g2_signature, g1_pubkey, kind }` enum variant definition

### Contract signing input spec
- `contracts/poa-middleware/contracts/src/bls/POAStakeRegistry.sol` — line ~240: `digest = keccak256(abi.encode(envelope))` → passed to `_checkSignatures` → then `HashToCurve.hashToCurveG2(abi.encodePacked(digest))`
- `contracts/poa-middleware/contracts/src/bls/libs/HashToCurve.sol` — DST constant `"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_"` (line 20); confirms signing input is the 32-byte digest as `bytes`

### Feature flag reference
- `packages/types/Cargo.toml` — `bls` feature currently opt-in; make default
- `packages/wavs/Cargo.toml` — `bls` feature; make default

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `bls_private_key_from_mnemonic(mnemonic, hd_index)` in `packages/utils/src/bls_signing.rs` — already implemented in Phase 5; returns `commonware_cryptography::bls12381::PrivateKey`
- `bls_g1_pubkey_bytes(pk)` in same file — returns `[u8; 128]` EIP-2537 G1 pubkey
- `WavsSignable::unprefixed_hash()` — returns `keccak256(abi_encode(envelope))` as `FixedBytes<32>`; this is exactly the `digest` the contract signs
- `tokio::task::spawn_blocking` — already used elsewhere in the codebase for CPU-bound work; same pattern needed here

### Established Patterns
- `WavsCryptoSigner::Secp256k1` path in `WavsSigner::sign()` — async, uses `pks.sign_hash(&hash).await`; BLS mirrors this but wraps blst in `spawn_blocking`
- `add_service_key` increments `signing_mnemonic_hd_index_count` atomically — same counter used for both secp256k1 and BLS indices
- `SignerInfo { signer: WavsCryptoSigner, hd_index: u32 }` struct — unchanged shape; BLS signer stored the same way

### Integration Points
- `dispatcher.rs` `add_service_to_managers()` — single call site for `add_service_key`; extend to read `service.submit` to get `SignatureKind` and pass `algorithm`
- `Submission { envelope_signature: WavsSignature }` — already supports BLS variant; no struct changes needed for P2P propagation
- `aggregator/p2p.rs` `P2pMessage` — JSON-serializes `Submission`; `WavsSignature::Bls12381` already serde-compatible (Phase 5 tests confirmed)

</code_context>

<specifics>
## Specific Ideas

- No specific UX references — this is a pipeline-internal change with no user-facing behavior change

</specifics>

<deferred>
## Deferred Ideas

- SignerResponse BLS variant for `/services/signer` HTTP endpoint — not discussed; handle in Phase 6 as Claude sees fit, or defer to Phase 8
- MCP tooling for BLS operator registration — explicitly deferred to v1.2 (per REQUIREMENTS.md)

</deferred>

---

*Phase: 06-bls-signing-pipeline*
*Context gathered: 2026-03-20*
