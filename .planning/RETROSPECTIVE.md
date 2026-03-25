# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — Commonware P2P Migration

**Shipped:** 2026-03-18
**Phases:** 4 | **Plans:** 11 | **Execution time:** ~2.8 hours (avg 15 min/plan)

### What Was Built
- Ed25519 P2P identity derived from BIP-39 mnemonic via ChaCha20Rng — deterministic across restarts
- Commonware-p2p lookup and discovery modes with Oracle authorization, rate limiting, and peer blocking
- commonware-broadcast buffered Engine with ServiceRouter filtering, SHA-256 dedup, retry queue, and catch-up
- New P2P config format (Disabled/Local/Remote) with configurable tuning fields; real connected-peer tracking in `/p2p/status`
- Full libp2p removal — zero workspace references after cleanup
- Complete documentation: P2P.md rewrite, announcement blog post, 231-line operator migration guide

### What Worked
- **TDD throughout**: Writing failing tests before implementation caught API assumption errors early (type mismatches, missing trait bounds)
- **Phase breakdown**: Separating identity/network, broadcast, config, and cleanup into discrete phases made progress predictable
- **Trait research upfront**: The commonware skill surfaced the `rand_chacha 0.3` / `rand_core 0.6` dependency constraint before it became a blocker
- **Dedicated OS thread pattern**: spawn_commonware_runtime() on std::thread with channel bridge worked cleanly — no Tokio nesting issues
- **SUMMARY.md discipline**: Capturing decisions and deviations in each plan made retrospective easy and will help future contributors

### What Was Inefficient
- **ROADMAP.md checkboxes not updated during execution** — all phases showed `[ ]` even after completion; the progress table diverged from reality (Phases 1-3 not checked off despite completion)
- **Audit needed manual gap investigation** — MISSING-01 (bootstrapper address format) was found in the audit rather than caught by e2e tests during Phase 4; earlier integration testing of the remote harness could have caught it sooner
- **CLI returned empty accomplishments** — `milestone complete` CLI couldn't extract accomplishments from SUMMARY.md files (fields returned as "None"); needed manual extraction

### Patterns Established
- `spawn_commonware_runtime()` on dedicated OS thread with `std::sync::mpsc` bridge to Tokio — canonical pattern for commonware-in-WAVS integration
- `rand_chacha = "0.3"` pinned for commonware compatibility (rand_core 0.6 boundary)
- `Config::local()` provides SEC-02 rate limiting by default — no explicit builder calls needed
- `Set::from_iter_dedup()` for discovery Oracle, `Map::from_iter_dedup()` for lookup Oracle — consistent dedup handling
- Two-channel broadcast (channel 0 = Engine cache, channel 1 = direct forward) for catch-up + live delivery

### Key Lessons
1. **Version boundaries matter for external crates**: commonware's `rand_core 0.6` transitive dep requires `rand_chacha 0.3`; pinning to exact compatible versions avoids trait incompatibility at compile time
2. **commonware-math is a direct dep**: `Random` trait for `PrivateKey::random()` lives in `commonware_math::algebra`, not re-exported — add it explicitly
3. **Milestone audits catch harness bugs**: The e2e test harness (handles.rs) had a silent bootstrapper address bug that unit/integration tests couldn't catch; milestone-level E2E audit is worth running before declaring done
4. **commonware channels are static**: Must be registered before `network.start()` — design for a fixed set of channels upfront, not dynamic per-service channels
5. **Oracle peer map format differs by mode**: Discovery uses `Set` (pubkeys only), Lookup uses `Map` (pubkey → address) — easy to mix up, document at the call site

### Cost Observations
- Model: claude-sonnet-4-6 (quality profile) throughout
- Sessions: ~5 sessions across 2 days (2026-03-17 to 2026-03-18)
- Notable: Most plans completed in 7-20 min; Phase 2 plan 02 was the longest (broadcast Engine wiring + 7 integration tests)

---

## Milestone: v1.1 — BLS Signatures

**Shipped:** 2026-03-23
**Phases:** 4 | **Plans:** 9 | **Execution time:** ~102 min (avg ~11 min/plan)

### What Was Built
- BLS12-381 ABI bindings with Alloy-generated Rust types, SignatureAlgorithm enum variant, WIT interface updates
- SignatureData/WavsSignature/WavsCryptoSigner converted from structs to enums — clean BLS/secp256k1 dispatch
- Deterministic BLS key derivation from mnemonic via HKDF-SHA256 with domain separation, G1 pubkey EIP-2537 conversion
- BLS signing pipeline: hash-to-curve with contract-matching DST, spawn_blocking for CPU-bound blst operations
- Algorithm-dispatched add_service_key, dispatcher auto-detection, SignerResponse::Bls12381 for HTTP API
- BLS G2 aggregation via blst point addition, keccak256-sorted G1 pubkeys, signer_identity() abstraction for dedup
- BLS EVM submission path with send_bls_envelope_signatures, BLS contract helpers, InsufficientQuorum error handling
- Full E2E test: PoaBlsMiddleware, SimpleBlsSubmit, 3-operator BLS quorum verified on Prague anvil with EIP-2537

### What Worked
- **Enum-based type migration** (Phase 5 plan 02): Converting SignatureData/WavsSignature/WavsCryptoSigner from structs to enums early gave all downstream phases clean pattern matching — every BLS vs secp256k1 dispatch was a match arm
- **Feature gating**: `cfg(feature = "bls")` kept BLS code optional in wavs-types, avoiding WASM compilation issues
- **Velocity improvement**: Average plan time dropped from 15 min (v1.0) to 11 min (v1.1) — familiarity with codebase patterns paid off
- **E2E test design**: Per-test middleware dispatch (EvmMiddlewares) enabled running BLS and secp256k1 tests in the same matrix without interference

### What Was Inefficient
- **ROADMAP.md checkboxes still not updated during execution** — same issue as v1.0; progress table diverged from actual plan completion status
- **blst FFI for G1 decompression**: Had to use raw blst FFI (blst_p1_uncompress + blst_p1_affine_serialize) because commonware didn't expose EIP-2537 format conversion — took extra research time
- **Duplicate bls_helpers**: Mirror of bls_helpers needed in wavs-types due to circular dep with layer-utils — tech debt
- **G2 coordinate ordering**: c0|c1 vs c1|c0 bug took debugging time; EIP-2537 expects a specific byte layout that differs from blst's internal ordering

### Patterns Established
- `SignatureAlgorithm` enum dispatch at every layer (signer creation, P2P submission, aggregation, EVM submission)
- HKDF-SHA256 with `WAVS-BLS-KEY-v1 || hd_index_le` domain separator for deterministic BLS key derivation
- `signer_identity()` abstraction for algorithm-generic queue dedup (EVM addr for secp256k1, keccak256(g1_pubkey) for BLS)
- `cfg_if!` inline gating for BLS RPC bindings — cleaner than separate modules
- Per-test middleware dispatch via `EvmMiddlewares` struct for mixed secp256k1/BLS test matrix

### Key Lessons
1. **blst DST matters critically**: Using commonware's Signer::sign() would produce signatures with POP suffix instead of RO — on-chain verification would silently fail. Always verify DST matches the contract.
2. **EIP-2537 byte layout**: G2 points use c0|c1 concatenation (not c1|c0). This is the opposite of some BLS libraries' internal ordering. Test with known test vectors.
3. **Enum migration early pays off**: Converting core types to enums before implementing algorithm-specific logic meant every subsequent phase had clean dispatch rather than if/else chains.
4. **Prague anvil default**: anvil 1.4.4+ defaults to Prague hardfork — explicitly passing --hardfork prague causes confusion when it fails differently than expected.
5. **Separate submit contracts**: BLS and ECDSA SignatureData are fundamentally different types (different struct fields) — a single ISimpleSubmit interface can't serve both.

### Cost Observations
- Model: mostly claude-sonnet-4-6, some opus for complex phases
- Sessions: ~4 sessions across 4 days (2026-03-19 to 2026-03-23)
- Notable: Plans 06-01 and 08-01 were fastest (6-7 min each); 07-02 was longest (21 min — BLS EVM submission wiring with type conversion complexity)

---

## Milestone: v1.2 — Tauri App

**Shipped:** 2026-03-25
**Phases:** 5 | **Plans:** 9 | **Tasks:** 16

### What Was Built
- Frontend type system widened for BLS: SignatureAlgorithm, P2pStatus, SignerResponse, BlsPubkeyResponse types + 3 new Tauri IPC commands
- Settings page refactored from 940-line monolith into 6 section components with sticky sidebar navigation
- P2P operator dashboard: Ed25519 identity card, connected peers list, subscribed services with operator key display and on-chain registration status
- BLS service builder: algorithm selector dropdown in SubmitEditor, post-deploy BLS G1 pubkey display, one-click BLS key registration via proof-of-possession
- Unified activity events: Rust backend error/success emission through Tauri IPC to Map-based Zustand correlation store with status progression
- Gap closure: SignaturePrefix type unified across 3 files, amber guidance banner for BLS services missing POA registry

### What Worked
- **Phase 9 as foundation**: Investing a full phase in types, Tauri commands, and settings decomposition before feature phases meant phases 10-12 could focus purely on UI features without structural blockers
- **Audit-driven gap closure**: The milestone audit (v1.2-MILESTONE-AUDIT.md) surfaced the SignaturePrefix type drift and missing BLS registration guidance, which became Phase 13 — catching real issues before shipping
- **Single-session phases**: Phases 10-13 each completed in a single session, maintaining momentum

### What Was Inefficient
- **Verification gaps accumulated**: Phases 9, 10, 12 shipped without VERIFICATION.md — the GSD verifier was disabled for speed, but this meant the milestone audit had to retroactively verify everything
- **Nyquist VALIDATION.md drafts never executed**: All 4 original phases have nyquist_compliant: false VALIDATION files that were created but never run through the auditor
- **P2P-06 quorum progress is placeholder**: The stretch goal for live quorum accumulation requires an `/aggregator/status` endpoint that doesn't exist — should have been deferred to Out of Scope earlier

### Patterns Established
- Tauri IPC command pattern: Rust `#[tauri::command]` → TypeScript wrapper in `utils/commands.ts` → Zustand store consumer
- Map-based correlation store for event matching: deterministic `correlationKey` (serviceId + triggerId + workflowIndex) for O(1) lookup
- BLS state lifting: BLS pubkey/registration status lifted to page level for cross-section sharing (rather than encapsulating in sub-components)
- Registration checks on mount + manual refresh only (avoids expensive on-chain reads in poll cycles)

### Key Lessons
1. **Foundation phases pay compound dividends**: The type system and Tauri command infrastructure from Phase 9 was reused by every subsequent phase without modification
2. **Audit before shipping catches real gaps**: The milestone audit found both a type inconsistency and a UX dead-end that would have confused operators
3. **Verification debt compounds**: Skipping phase verification for speed meant 15/19 requirements had only partial (summary-claimed) verification at audit time
4. **Stretch goals should be scoped earlier**: P2P-06 should have been moved to Out of Scope during requirements rather than carried as a placeholder through execution

### Cost Observations
- Model: opus for execution, sonnet for verification
- Sessions: ~3 sessions across 2 days (2026-03-24 to 2026-03-25)
- Notable: Phase 13 (gap closure) was fastest — 1 plan, 2 tasks, completed in ~4 min

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Avg/Plan | Key Change |
|-----------|--------|-------|----------|------------|
| v1.0 | 4 | 11 | 15 min | First milestone — established WAVS+commonware patterns |
| v1.1 | 4 | 9 | 11 min | BLS signatures — enum dispatch, blst integration, E2E BLS |
| v1.2 | 5 | 9 | ~10 min | Tauri app — frontend types, P2P dashboard, BLS UX, activity events |

### Cumulative Quality

| Milestone | Tests Added | Requirements | Notable |
|-----------|-------------|--------------|---------|
| v1.0 | 28 | 27/27 | libp2p fully removed |
| v1.1 | 3 (BLS E2E) | 14/14 | BLS + secp256k1 coexistence verified |
| v1.2 | 0 (UI) | 19/19 | Tauri desktop app fully synced with backend |

### Top Lessons (Verified Across Milestones)

1. TDD (failing tests first) consistently catches API surface mismatches before implementation sinks too deep
2. External crate version pinning is critical for embedded runtimes with transitive rand/crypto deps
3. ROADMAP.md checkboxes fall behind during execution — consider automating this via gsd-tools post-plan hooks
4. Enum-based type design enables clean multi-algorithm dispatch without if/else proliferation
5. Foundation phases (types, commands, infrastructure) before feature phases yields compound returns — every v1.2 feature phase reused Phase 9 work
6. Milestone audits catch real shipping gaps — both v1.0 and v1.2 benefited from pre-ship audits that found issues automated checks missed
