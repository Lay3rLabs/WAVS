# Phase 1: Secure Peer Connectivity - Context

**Gathered:** 2026-03-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Establish the authenticated peer connection layer for WAVS: derive a deterministic Ed25519 identity from the operator's mnemonic, bring up commonware-p2p networking (lookup mode and discovery mode), and enforce Oracle-based operator authorization. Broadcast, message routing, the `P2pHandle` API, and P2pStatus are out of scope — those land in Phase 2.

</domain>

<decisions>
## Implementation Decisions

### Code Organization
- Replace `packages/wavs/src/subsystems/aggregator/p2p.rs` in-place (do not create a parallel module)
- Aggregator may be non-functional mid-migration; this is acceptable — Phase 1 lives on a branch that does not merge to `main` until Phase 2 is complete
- Delete libp2p code as each component is replaced by its commonware equivalent — forward progress only, no keeping dead code around

### Test Placement
- Phase 1 tests live in `packages/wavs/tests/` (Rust integration tests)
- Tests spin up the P2P connection layer in isolation — no Dispatcher or Aggregator involved
- Full-stack e2e validation is Phase 4's concern (`just test-wavs-e2e`)

### Discovery Mode Order
- Implement `lookup` mode (known peer addresses) first — simpler, no bootstrapper node needed, localhost testing is trivial
- Implement `discovery` mode (bootstrapper-based) second
- Local dev uses `lookup` mode with explicit peer addresses (not `discovery::Config::local()`)

### Oracle Peer Authorization
- New config field `authorized_peers` in the P2P section of `wavs.toml`
- Format: flat array of Ed25519 hex pubkeys — `authorized_peers = ["aabbcc...", "ddeeff..."]`
- The local node's own pubkey is implicitly trusted — operators do not need to list themselves

### Claude's Discretion
- Whether the node's own pubkey should appear in Oracle `track()` calls (likely no — Oracle manages other peers)
- Exact Rust module structure inside `p2p.rs` as it's being replaced (helper functions, sub-structs)
- Ed25519 key derivation specifics: use `ChaCha20Rng::from_seed(bip39_seed[..32])` + `ed25519::PrivateKey::random(&mut rng)` per STACK.md recommendation; the domain/namespace labeling is Claude's call
- Integration test harness setup (port allocation, cleanup, test timeouts)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` — Phase 1 goal, success criteria (5 items), and requirement IDs (IDEN-01, IDEN-02, NET-01–04, SEC-01–03)
- `.planning/REQUIREMENTS.md` — Full requirement specs for IDEN-01/02, NET-01–04, SEC-01–03

### Implementation patterns (HIGH priority — read before writing code)
- `.planning/research/STACK.md` — Exact Ed25519 derivation code pattern (ChaCha20Rng + PrivateKey::random), all 5 commonware crate versions (pin at `2026.3.0`), runtime integration pseudocode, channel registration pattern
- `.planning/research/ARCHITECTURE.md` — Component boundaries diagram, P2pHandle preservation contract, detailed build order for Phase 1 and 2, data flow diagrams
- `.planning/research/PITFALLS.md` — Pitfall 2 (runtime ownership conflict, CRITICAL for Phase 1), Pitfall 4 (identity scheme change, breaking cutover), Pitfall 9 (ALPHA stability, pin versions)

### Existing code to replace
- `packages/wavs/src/subsystems/aggregator/p2p.rs` — The 1,840-line file being replaced in-place; contains `keypair_from_mnemonic()` (to be replaced by `ed25519_signer_from_mnemonic()`), `P2pConfig` enum, `P2pHandle`, `P2pCommand`, `EventLoopState`

### External API surfaces (do not break)
- `packages/types/src/http.rs` — `P2pStatus` struct fields; Phase 1 does not update this, but Phase 2 will; understand the contract
- `packages/wavs/src/subsystems/aggregator/aggregator.rs` — Holds `p2p_handle: Arc<RwLock<Option<P2pHandle>>>`, unchanged in Phase 1

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `keypair_from_mnemonic()` in `p2p.rs` — existing mnemonic derivation function; Phase 1 replaces this with `ed25519_signer_from_mnemonic()` using the same `bip39` crate already in the workspace
- `P2pConfig` enum (Disabled / Local / Remote) — enum structure stays, field contents change; existing serde attributes can be reused
- `PendingPublish` / retry queue logic in `p2p.rs` — Phase 2 concern but do not delete; it stays until replaced
- `packages/layer-tests/` — reference for integration test patterns (test setup, port allocation via `DEFAULT_P2P_BASE_PORT = 9000`)

### Established Patterns
- Crossbeam channels for subsystem communication — the `P2pHandle` uses `tokio::sync::mpsc::UnboundedSender<P2pCommand>`; the new commonware coordinator task bridges this channel
- `std::thread::spawn` for blocking subsystems — commonware's `Runner` must run on a dedicated OS thread (see STACK.md runtime integration pattern); WAVS already uses this pattern for other blocking work
- Workspace `Cargo.toml` for shared dependency versions — add commonware crates at workspace level if other crates will use them, otherwise just in `packages/wavs/Cargo.toml`

### Integration Points
- `packages/wavs/src/subsystems/aggregator/aggregator.rs` — creates `P2pHandle` during startup; Phase 1 changes how the handle is initialized internally but not the handle interface itself
- `packages/wavs/src/dispatcher.rs` — passes `P2pConfig` from `wavs.toml` to the aggregator; the new `authorized_peers` field will be added to the config struct read here

</code_context>

<specifics>
## Specific Ideas

No specific UX references beyond what's in the requirements and research docs.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-secure-peer-connectivity*
*Context gathered: 2026-03-17*
