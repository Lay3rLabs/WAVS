# Milestones

## v1.1 BLS Signatures (Shipped: 2026-03-23)

**Phases completed:** 4 phases, 9 plans, 16 tasks

**Key accomplishments:**

- BLS12-381 ABI bindings generating Rust types via alloy_sol_macro, SignatureAlgorithm::Bls12381 variant compiling and serializing as "bls12381", WIT interfaces updated in all three locations
- SignatureData, WavsSignature, and WavsCryptoSigner converted to enums with full workspace migration -- secp256k1 path unchanged, BLS stubs ready for Phase 6
- Deterministic BLS12-381 key derivation from mnemonic+HD index using HKDF-SHA256, with G1 pubkey conversion to 128-byte EIP-2537 format via blst FFI
- BLS12-381 signing with contract-matching DST (BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_), producing 256-byte EIP-2537 G2 signatures via blst with WavsSigner::sign() BLS arm wired through spawn_blocking
- Algorithm-dispatched add_service_key() with BLS signer creation, dispatcher auto-detection from service config, SignerResponse::Bls12381 for graceful HTTP API, bls feature default-on in packages/wavs
- BLS G2 signature aggregation via blst point addition with keccak256-sorted G1 pubkeys, plus algorithm-generic queue dedup replacing evm_signer_address
- BLS EVM submission path wired end-to-end: send_bls_envelope_signatures() with retry logic, BLS contract helpers, and SignatureData::Bls12381 dispatch in handle_action_submit_evm()
- PoaBlsMiddleware deploying BLS contracts via local forge, Prague-capable anvil, SimpleBlsSubmit.sol, and BLS-aware AvsOperator

**Git range:** `9fc6a5fd` (feat(05-01)) → `e508bcf6` (feat(08-02))
**Lines:** +9,362 / -550 across 122 files
**Timeline:** 4 days (2026-03-19 → 2026-03-23)
**Archive:** [milestones/v1.1-ROADMAP.md](milestones/v1.1-ROADMAP.md)

---

## v1.0 Commonware P2P Migration (Shipped: 2026-03-18)

**Phases completed:** 4 phases, 11 plans

**Key accomplishments:**

1. Replaced libp2p secp256k1 identity with Ed25519 derived from BIP-39 mnemonic via ChaCha20Rng -- deterministic peer IDs across node restarts (IDEN-01, IDEN-02)
2. Implemented commonware-p2p lookup and discovery modes with Oracle peer authorization, rate limiting, and BlockPeer support -- all security requirements satisfied (NET-01 to NET-04, SEC-01 to SEC-03)
3. Implemented P2pMessage with Codec+Digestible traits and wired commonware-broadcast buffered Engine -- broadcast, service filtering, dedup, retry, and catch-up all working (BCAST-01 to BCAST-05, CATCH-01, CATCH-02)
4. Rewrote P2P config to Disabled/Local/Remote format with configurable max_message_size and deque_size; status endpoint returns real connected peer tracking (CFG-01 to CFG-03, OBS-01, OBS-02)
5. Removed libp2p 0.56 from workspace entirely; renamed test harness enums (Local/Remote); all e2e tests pass (INT-01 to INT-03)
6. Complete documentation: P2P.md rewritten for commonware, announcement blog post, and 231-line operator migration guide covering all 4 breaking changes (DOC-01 to DOC-03)

**Git range:** `3e101d79` (feat: Ed25519 identity) -> `08748210` (Tests pass)
**Archive:** [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md)

---
