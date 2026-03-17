# Phase 4: Validation and Cleanup - Research

**Researched:** 2026-03-17
**Domain:** E2E test validation, dependency removal, technical documentation
**Confidence:** HIGH

## Summary

Phase 4 is the final phase of the commonware P2P migration. All code changes for the commonware backend are complete (Phases 1-3 delivered identity, networking, broadcast, config, and observability). This phase validates the migration end-to-end, removes all legacy dependencies (libp2p, hypercore, hyperswarm), updates documentation to reflect the new architecture, and produces operator-facing migration guidance.

The primary risk is the libp2p/hypercore/hyperswarm removal -- these dependencies are deeply embedded in the trigger subsystem (hypercore streams), the e2e test harness (HypercoreTestClient, hyperswarm bootstrap), and several Cargo.toml files. Removal requires understanding which code is truly dead vs still active. The e2e test infrastructure already uses the new commonware P2pConfig for multi-operator tests, but several test files still import hypercore/hyperswarm for the Hypercore trigger type (which is separate from the P2P aggregator migration).

**Primary recommendation:** Split this phase into three plans: (1) E2E test validation and test harness cleanup, (2) libp2p dependency removal and dead code cleanup, (3) documentation (P2P.md rewrite, blog post, migration guide).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INT-02 | All existing e2e tests pass (`just test-wavs-e2e`) | E2E test infrastructure analysis below; test harness uses commonware P2pConfig already; `layer-tests.toml` p2p modes need renaming from mdns/kademlia to local/remote |
| INT-03 | libp2p dependency removed from Cargo.toml | Dependency tree analysis below; libp2p referenced in root Cargo.toml (workspace dep) and packages/wavs/Cargo.toml; zero Rust source files import libp2p |
| DOC-01 | `docs/P2P.md` updated with commonware setup, config examples, multi-node instructions | Current P2P.md fully documents the OLD libp2p architecture; needs complete rewrite to match commonware |
| DOC-02 | Blog post in `docs/blog/` announcing commonware integration | No `docs/blog/` directory exists yet; needs creation |
| DOC-03 | Operator migration guide documenting identity change, config format change, coordinated upgrade requirement | Key changes: secp256k1 -> Ed25519, multiaddr -> socket addresses, mDNS/Kademlia -> Local/Remote modes, GossipSub topics -> single broadcast channel |
</phase_requirements>

## Standard Stack

No new libraries needed for this phase. This phase removes dependencies and writes documentation.

### Dependencies to REMOVE

| Library | Location | Reason for Removal |
|---------|----------|-------------------|
| `libp2p` (v0.56, 13 feature flags) | Root `Cargo.toml` workspace dep + `packages/wavs/Cargo.toml` | Replaced by commonware-p2p. Zero `.rs` files import it. |
| `hypercore` (v0.14.0) | Root `Cargo.toml` + `packages/wavs/Cargo.toml` + `packages/layer-tests/Cargo.toml` | Used only by trigger subsystem's hypercore stream. IMPORTANT: This is the Hypercore TRIGGER type, not P2P. Evaluate whether to keep or remove. |
| `hypercore-protocol` (v0.6.1) | Root `Cargo.toml` + `packages/wavs/Cargo.toml` | Same as hypercore -- trigger subsystem dependency. |
| `hyperswarm` (git dep, rev da1c2fd) | Root `Cargo.toml` + `packages/wavs/Cargo.toml` + `packages/layer-tests/Cargo.toml` | Used by hypercore stream for peer discovery. If hypercore triggers remain, hyperswarm stays. |
| `async-std` (v1) | `packages/layer-tests/Cargo.toml` | Only used by hyperswarm bootstrap code (commented out). Remove if hyperswarm goes. |

### CRITICAL DISTINCTION: libp2p vs hypercore/hyperswarm

**libp2p** is the P2P networking library for the Aggregator subsystem. It has been fully replaced by commonware. Zero Rust source files reference it. **This MUST be removed.**

**hypercore/hyperswarm** are used by the Trigger subsystem for Hypercore append triggers (a separate feature from P2P aggregation). These MAY still be needed if the Hypercore trigger type is active. The success criteria says "libp2p and all 13 of its feature flags are removed" -- it does NOT mention hypercore/hyperswarm. However, the requirement says "zero libp2p references in the dependency tree" which is specifically about libp2p.

**Recommendation:** Remove libp2p (confirmed dead). Keep hypercore/hyperswarm IF the Hypercore trigger type is still active. If it's also deprecated (the test registration code is commented out), remove it too and clean up the trigger subsystem. The planner should treat hypercore removal as a separate decision.

## Architecture Patterns

### E2E Test Infrastructure

The e2e test system in `packages/layer-tests/` uses:

```
packages/layer-tests/
  src/
    config.rs              # TestP2pMode enum (Mdns, Kademlia)
    e2e/
      config.rs            # Configs struct, P2pConfig construction
      handles.rs           # AppHandles: WAVS operator startup, P2P bootstrap
      handles/
        hypercore.rs       # HypercoreTestClient (hyperswarm-based)
        evm.rs
        cosmos.rs
      runner.rs            # Test runner with hypercore client setup
      test_registry.rs     # Test definitions including multi_operator
      matrix.rs            # Test matrix (which tests to run)
      helpers.rs           # wait_for_hypercore_mesh_ready etc.
  layer-tests.toml         # Config file: p2p = "kademlia"
```

### Key Test Harness Patterns

**Multi-operator test startup (Remote/Kademlia mode):**
1. Operator 0 starts first as bootstrap server
2. Test waits for `/p2p/status` to return a listen address
3. Remaining operators start with operator 0's address as bootstrapper
4. All operators configured via `P2pConfig::Remote` from the new commonware config

**Multi-operator test startup (Local/Mdns mode):**
1. All operators start simultaneously
2. Each gets `P2pConfig::Local` with listen_port only
3. No explicit bootstrapping needed

**TestP2pMode rename needed:**
- `Mdns` -> rename to match `Local` (commonware terminology)
- `Kademlia` -> rename to match `Remote` (commonware terminology)
- `layer-tests.toml`: `p2p = "kademlia"` -> `p2p = "remote"` (or `"lookup"/"discovery"`)

### P2pConfig Construction Issue in Tests

The `e2e/config.rs` constructs `P2pConfig::Remote` and `P2pConfig::Local` without the `max_message_size` and `deque_size` fields added in Phase 3. These are `Option<T>` fields. In Rust, enum variant construction requires ALL fields. This may be a latent compile error masked by incremental compilation cache. The test harness needs updating to include `max_message_size: None, deque_size: None` or use a builder pattern.

Similarly, `e2e/handles.rs` destructures `P2pConfig::Remote` without these optional fields (lines 231-235).

### Documentation Structure

Current docs requiring updates:
```
docs/
  P2P.md                 # FULL REWRITE needed
  ARCHITECTURE.md        # Line 80: mentions GossipSub, mDNS, Kademlia
  LOCAL_DEV.md           # May reference old P2P config
  blog/                  # Does not exist -- create
```

CLAUDE.md references:
- Line 82: "Uses libp2p for P2P trigger distribution" -- update
- Line 137: "libp2p/Hyperswarm networking" -- update

### wavs.toml Config Template

The `wavs.toml` file (root) already has the commonware P2P config section (lines 192-243) from Phase 3 work. The old hyperswarm_bootstrap setting on line 178-179 needs removal IF hypercore triggers are removed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dependency tree analysis | Manual grep for transitive deps | `cargo tree -p libp2p` | Shows full transitive dependency tree, confirms clean removal |
| Compilation verification | Incremental `cargo check` | `cargo check --release` or `cargo build -p wavs -p layer-tests` after cache clean | Incremental compilation can mask errors |
| Documentation formatting | Freeform markdown | Follow existing `docs/P2P.md` section structure | Consistency with existing docs |

## Common Pitfalls

### Pitfall 1: Removing hypercore/hyperswarm when it's still used by triggers
**What goes wrong:** The Hypercore trigger type (`TriggerData::HypercoreAppend`) is a live feature, not part of the P2P migration. Removing hypercore/hyperswarm breaks the trigger subsystem.
**Why it happens:** Confusion between "P2P networking for aggregation" (libp2p, now commonware) and "Hypercore data streams for triggers" (hypercore/hyperswarm, separate system).
**How to avoid:** Check if Hypercore trigger tests are active (they're currently commented out in test_registry.rs). If the trigger type is still in production use, keep those dependencies.
**Warning signs:** `packages/wavs/src/subsystems/trigger/streams/hypercore_stream.rs` still has active code, not commented out.

### Pitfall 2: Incremental compilation hiding missing fields
**What goes wrong:** `P2pConfig::Remote { listen_port, bootstrappers, authorized_peers }` construction compiles locally but fails in CI or clean builds because `max_message_size` and `deque_size` fields are missing.
**Why it happens:** Rust incremental compilation caches compiled artifacts; the dependent crate may not be recompiled.
**How to avoid:** Run `cargo clean -p layer-tests && cargo check -p layer-tests` to verify.
**Warning signs:** CI failures that don't reproduce locally.

### Pitfall 3: TestP2pMode serde rename breaking config files
**What goes wrong:** Renaming `Mdns`/`Kademlia` to `Local`/`Remote` breaks deserialization of `layer-tests.toml` which has `p2p = "kademlia"`.
**Why it happens:** `#[serde(rename_all = "snake_case")]` means the TOML value must match the new variant name.
**How to avoid:** Update `layer-tests.toml` simultaneously with the enum rename. Consider adding `#[serde(alias = "kademlia")]` for backward compatibility if needed.
**Warning signs:** E2E test startup panics with deserialization error.

### Pitfall 4: Blog post and migration guide tone mismatch
**What goes wrong:** Blog post reads like a tutorial, migration guide reads like an announcement.
**Why it happens:** Not following the stated requirements carefully.
**How to avoid:** Blog post = announcement style (what changed, why, high-level impact). Migration guide = operational (step-by-step: what to change, identity format differences, config migration, coordinated upgrade).

### Pitfall 5: Forgetting CLAUDE.md references
**What goes wrong:** `CLAUDE.md` (project instructions for AI) still references libp2p/Hyperswarm, causing AI assistants to give outdated guidance.
**Why it happens:** CLAUDE.md is not in `docs/` so it's easy to miss.
**How to avoid:** Search CLAUDE.md for libp2p references and update them.

## Code Examples

### Example: Removing libp2p from workspace Cargo.toml

```toml
# REMOVE this entire block from root Cargo.toml (lines ~238-253):
# libp2p = { version = "0.56", features = [
#     "tokio",
#     "tcp",
#     "dns",
#     "noise",
#     "yamux",
#     "identify",
#     "ping",
#     "gossipsub",
#     "request-response",
#     "kad",
#     "mdns",
#     "autonat",
#     "macros",
#     "secp256k1",
# ] }

# REMOVE from packages/wavs/Cargo.toml:
# libp2p = { workspace = true }
```

### Example: Fixing P2pConfig construction in e2e/config.rs

```rust
// BEFORE (missing optional fields):
wavs_config.p2p = P2pConfig::Remote {
    listen_port: DEFAULT_P2P_BASE_PORT + operator_index as u16,
    bootstrappers: vec![],
    authorized_peers: vec![],
};

// AFTER (all fields specified):
wavs_config.p2p = P2pConfig::Remote {
    listen_port: DEFAULT_P2P_BASE_PORT + operator_index as u16,
    bootstrappers: vec![],
    authorized_peers: vec![],
    max_message_size: None,
    deque_size: None,
};
```

### Example: Fixing P2pConfig destructuring in e2e/handles.rs

```rust
// BEFORE (missing fields in pattern):
if let P2pConfig::Remote {
    listen_port,
    bootstrappers: _,
    authorized_peers,
} = &config.p2p

// AFTER (use .. to ignore optional fields):
if let P2pConfig::Remote {
    listen_port,
    bootstrappers: _,
    authorized_peers,
    ..
} = &config.p2p
```

### Example: TestP2pMode rename

```rust
// BEFORE:
pub enum TestP2pMode {
    #[default]
    Mdns,
    Kademlia,
}

// AFTER:
pub enum TestP2pMode {
    #[default]
    Local,
    Remote,
}
```

### Example: Updated P2P.md structure

```markdown
# P2P Networking

## Overview
P2P networking is an optional feature of the Aggregator subsystem...
Uses commonware-p2p for authenticated peer networking with Ed25519 identities.

## Architecture
- Two modes: Lookup (local dev) and Discovery (production)
- Single broadcast channel with application-level service filtering
- Ed25519 identity derived from WAVS_SIGNING_MNEMONIC

## Configuration
### Disabled (default)
### Local -- Lookup Mode (development)
### Remote -- Discovery Mode (production)

## Multi-Node Setup
### Local Development (2 operators)
### Production Deployment

## Status Endpoint
GET /p2p/status returns: peer_id, listen_addresses, connected_peers, subscribed_services

## Identity
Ed25519 keypair derived via ChaCha20Rng from BIP-39 mnemonic
```

### Example: Migration guide key sections

```markdown
# Operator Migration Guide: libp2p to commonware

## Breaking Changes
1. Identity: secp256k1 -> Ed25519 (peer IDs change)
2. Address format: multiaddr -> socket format (<pubkey>@<host>:<port>)
3. Config: [wavs.p2p] local/remote replaces mdns/kademlia
4. Discovery: mDNS replaced by lookup mode, Kademlia by discovery mode

## Coordinated Upgrade Requirement
All operators in a deployment must upgrade simultaneously.
Old (libp2p) and new (commonware) nodes cannot communicate.

## Step-by-Step Migration
1. Stop all operator nodes
2. Update binary to new version
3. Update wavs.toml P2P configuration
4. Generate new Ed25519 identity: `wavs-cli p2p identity --mnemonic "..."`
5. Exchange new peer IDs with other operators
6. Start all operator nodes
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| libp2p 0.56 with GossipSub | commonware-p2p 2026.3.0 with broadcast Engine | Phase 1-3 (this migration) | All P2P code rewritten |
| secp256k1 peer identity | Ed25519 peer identity | Phase 1 | Peer IDs incompatible |
| multiaddr format | Socket address + hex pubkey | Phase 1 | Address format change |
| Per-service GossipSub topics | Single channel + ServiceRouter filtering | Phase 2 | Simpler architecture |
| mDNS / Kademlia DHT | Lookup mode / Discovery mode | Phase 1 | Config terminology change |

## Inventory of Files Requiring Changes

### INT-03: libp2p Removal (code changes)

| File | Change | Impact |
|------|--------|--------|
| `Cargo.toml` (root) | Remove `libp2p = { ... }` from `[workspace.dependencies]` | Workspace dep gone |
| `packages/wavs/Cargo.toml` | Remove `libp2p = { workspace = true }` | Package dep gone |
| `Cargo.lock` | Auto-updated after Cargo.toml changes | Transitive deps gone |

### INT-02: E2E Test Fixes (code changes)

| File | Change | Impact |
|------|--------|--------|
| `packages/layer-tests/src/config.rs` | Rename `TestP2pMode::Mdns` -> `Local`, `Kademlia` -> `Remote` | Test config enum |
| `packages/layer-tests/layer-tests.toml` | `p2p = "kademlia"` -> `p2p = "remote"` | Test config file |
| `packages/layer-tests/src/e2e/config.rs` | Add `max_message_size: None, deque_size: None` to P2pConfig constructions | Fix potential compile error |
| `packages/layer-tests/src/e2e/handles.rs` | Add `..` to P2pConfig::Remote destructuring; remove `_hyperswarm_bootstrap` field; remove commented hyperswarm code | Test harness cleanup |
| `packages/layer-tests/src/e2e/handles.rs` | Remove `async_std` import if hyperswarm bootstrap removed | Cleanup |

### DOC-01/02/03: Documentation

| File | Change | Impact |
|------|--------|--------|
| `docs/P2P.md` | Complete rewrite | Operator-facing docs |
| `docs/ARCHITECTURE.md` | Update line 80 (P2P Networking section) | Architecture docs |
| `CLAUDE.md` | Update lines 82, 137 | AI guidance |
| `docs/blog/` (new dir) | Create blog post | Announcement |
| `docs/` (new file) | Create migration guide | Operator guidance |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | `packages/layer-tests/layer-tests.toml` |
| Quick run command | `cargo check -p wavs -p layer-tests` |
| Full suite command | `just test-wavs-e2e` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| INT-02 | E2E tests pass with commonware backend | e2e | `just test-wavs-e2e` | Existing tests |
| INT-03 | libp2p removed from dependency tree | build verification | `cargo tree -p wavs 2>&1 \| grep -c libp2p` (expect 0) | N/A (build check) |
| DOC-01 | P2P.md updated | manual review | `grep -c "commonware\|Ed25519\|lookup\|discovery" docs/P2P.md` (expect >0) | docs/P2P.md exists |
| DOC-02 | Blog post exists | manual review | `test -f docs/blog/*.md` | Does not exist yet |
| DOC-03 | Migration guide exists | manual review | `grep -c "migration\|upgrade\|breaking" docs/MIGRATION.md` | Does not exist yet |

### Sampling Rate
- **Per task commit:** `cargo check -p wavs -p layer-tests`
- **Per wave merge:** `just test-wavs-e2e` (requires full stack: anvil, WAVS nodes)
- **Phase gate:** Full e2e suite green + `cargo tree -p wavs | grep libp2p` returns nothing

### Wave 0 Gaps
None -- existing test infrastructure (layer-tests, cargo check) covers all phase requirements. No new test files needed.

## Open Questions

1. **Should hypercore/hyperswarm be removed too?**
   - What we know: Hypercore triggers are a separate feature from P2P aggregation. The Hypercore trigger test registration is commented out in `test_registry.rs`. The hypercore stream code in `packages/wavs/src/subsystems/trigger/streams/` is active (not commented). The `wavs.toml` still has `hyperswarm_bootstrap` config.
   - What's unclear: Is the Hypercore trigger type actively used in production, or is it deprecated?
   - Recommendation: Keep hypercore/hyperswarm for now (they're separate from the P2P migration scope). Only remove if explicitly confirmed deprecated. The success criteria says "libp2p removed" not "hypercore removed."

2. **TestP2pMode naming: should it use `Local`/`Remote` or `Lookup`/`Discovery`?**
   - What we know: P2pConfig uses `Local`/`Remote`. Internally, Local = lookup mode, Remote = discovery mode.
   - What's unclear: Whether test config should match the user-facing config names or the internal mode names.
   - Recommendation: Use `Local`/`Remote` to match `P2pConfig` variant names.

3. **Should the `max_message_size` and `deque_size` missing-field issue be fixed in the e2e tests?**
   - What we know: P2pConfig construction in e2e tests is missing these Optional fields. The code currently compiles (possibly via incremental cache).
   - What's unclear: Whether this is actually a compile error or if Rust handles Option fields differently than expected.
   - Recommendation: Add the fields explicitly with `None` values to be safe, regardless of current compile status.

## Sources

### Primary (HIGH confidence)
- Direct codebase analysis of `packages/wavs/src/subsystems/aggregator/p2p.rs` -- P2pConfig struct, all Phase 1-3 work
- Direct codebase analysis of `packages/layer-tests/` -- Full e2e test infrastructure
- Direct codebase analysis of `Cargo.toml` (root + packages) -- Dependency tree
- Direct codebase analysis of `docs/P2P.md` -- Current documentation state
- Direct codebase analysis of `wavs.toml` -- Current config template
- `cargo check -p layer-tests` -- Verified current compilation status

### Secondary (MEDIUM confidence)
- `.planning/STATE.md` -- Project decisions and accumulated context from Phases 1-3
- `.planning/REQUIREMENTS.md` -- Requirement definitions and traceability
- `.planning/ROADMAP.md` -- Phase dependencies and success criteria

## Metadata

**Confidence breakdown:**
- Standard stack (deps to remove): HIGH -- direct Cargo.toml analysis, zero ambiguity on libp2p removal
- Architecture (test harness): HIGH -- full code read of e2e test infrastructure
- Pitfalls: HIGH -- identified from actual code analysis, not hypothetical
- Documentation scope: HIGH -- enumerated all files with stale references
- Hypercore/hyperswarm decision: MEDIUM -- unclear if actively used in production

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (stable -- all findings based on current codebase state, no external dependencies to go stale)
