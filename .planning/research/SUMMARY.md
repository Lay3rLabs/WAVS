# Project Research Summary

**Project:** WAVS Commonware P2P Migration
**Domain:** P2P networking layer replacement (libp2p 0.56 -> commonware 2026.3.0)
**Researched:** 2026-03-17
**Confidence:** MEDIUM

## Executive Summary

WAVS is replacing its libp2p-based P2P networking layer with commonware, an "anti-framework" suite of Rust crates designed for authenticated, known-operator networks. The migration scope is tightly bounded: the change is entirely within `packages/wavs/src/subsystems/aggregator/p2p.rs` (~1,840 lines), behind the existing `P2pHandle` abstraction that the rest of the codebase depends on. The recommended approach is a 6-phase incremental rewrite — starting with cryptographic identity, then the network skeleton, then broadcast, then service routing, then config migration, and finally e2e validation and libp2p removal. The `P2pHandle` interface (publish, subscribe, unsubscribe, get_status) is preserved throughout, meaning the Aggregator and Dispatcher see no changes.

The key architectural decision is how to bridge commonware's static-channel model against WAVS's dynamic service subscriptions. Commonware channels must be registered before `network.start()` and cannot be added at runtime, which conflicts with WAVS's `SubscribeService`/`UnsubscribeService` commands. The recommended resolution is a single broadcast channel for all services with application-level filtering by `service_id` — this sidesteps the static channel constraint entirely and is the simplest correct approach. The second key decision is runtime integration: commonware's `Runner` creates its own Tokio runtime internally, requiring WAVS to run it on a dedicated `std::thread` and bridge via cross-thread channels.

The main risks are: (1) commonware is ALPHA software with explicit breaking-change warnings — pin versions exactly and preserve the `P2pHandle` abstraction as an escape hatch; (2) the runtime integration strategy (dedicated thread + separate Tokio runtime) is sound in theory but needs early prototype validation before building P2P logic on top; (3) the identity scheme change from secp256k1 to Ed25519 is a hard cutover requiring all operators to upgrade simultaneously. None of these risks are blockers, but they must be addressed in the correct phase order to avoid rewrites.

## Key Findings

### Recommended Stack

Replace all 13 libp2p feature flags with five commonware crates pinned at `2026.3.0` (CalVer). The crates are published in lockstep from the commonware monorepo, so version management is straightforward. The main addition beyond commonware is `rand_chacha` for deterministic Ed25519 key derivation from the existing BIP-39 mnemonic. All existing WAVS dependencies (tokio, crossbeam, serde, tracing, axum) are unchanged.

**Core technologies:**
- `commonware-p2p 2026.3.0`: Authenticated peer networking — replaces libp2p Swarm, Kademlia, mDNS, Noise, yamux, Identify, AutoNAT
- `commonware-broadcast 2026.3.0`: Message dissemination and caching — replaces GossipSub and the entire custom catch-up protocol
- `commonware-cryptography 2026.3.0`: Ed25519 key generation and signing — replaces libp2p secp256k1 P2P identity
- `commonware-runtime 2026.3.0`: Async runtime abstraction — required dependency of commonware-p2p; runs on a dedicated thread
- `commonware-codec 2026.3.0`: Binary serialization — used by commonware-p2p internally; `Submission` needs `Encode + Decode` implementations
- `rand_chacha 0.9+`: Deterministic RNG — seeds Ed25519 key derivation from BIP-39 mnemonic entropy

See `.planning/research/STACK.md` for full dependency mapping and the runtime integration code pattern.

### Expected Features

The migration must preserve all existing P2P capabilities while adopting commonware's different primitives. The hardest mapping problem is dynamic service subscriptions against commonware's static channels — resolved by single-channel broadcast with service-ID filtering. The loss of mDNS for local dev is a minor DX change addressed by `lookup` mode with known addresses.

**Must have (table stakes):**
- Per-service message broadcast — `commonware-broadcast::buffered::Engine` replaces GossipSub; single channel, filter by `service_id`
- Peer discovery (production) — `authenticated::discovery` with bootstrappers replaces Kademlia DHT
- Peer discovery (local dev) — `authenticated::lookup` with known addresses replaces mDNS
- Deterministic P2P identity from mnemonic — Ed25519 key derived from existing `WAVS_SIGNING_MNEMONIC` via ChaCha20Rng
- Encrypted peer connections — built into commonware authenticated modules, no separate config needed
- Missed message retrieval on reconnect — `buffered::Engine` digest-based retrieval replaces custom `CatchUpRequest`/`CatchUpResponse` protocol
- `/p2p/status` endpoint — reconstructed from commonware Oracle and config primitives; address format changes from multiaddr to socket address
- P2pHandle API preserved — `publish`, `subscribe`, `unsubscribe`, `get_status` interface stays identical

**Should have (enabled by commonware that libp2p did not provide):**
- Oracle-based authorized peer sets — register only known operators; security improvement over open GossipSub
- Built-in per-peer and per-subnet rate limiting — DoS protection at the network layer
- Peer blocking by cryptographic identity — block misbehaving peers across IP changes
- Namespace-scoped replay protection — `discovery::Config::namespace` prevents cross-network message replay
- Simulated networking for tests — `commonware-p2p::simulated` enables deterministic unit tests without real network

**Defer (v2+):**
- On-chain operator registry integration with Oracle — start with static peer sets from config
- Priority message support — tune after observing production behavior
- NAT traversal infrastructure — require operators to configure `dialable` address; no AutoNAT equivalent in commonware

See `.planning/research/FEATURES.md` for full feature dependency chain and anti-features to avoid.

### Architecture Approach

The migration is a contained replacement inside the existing `P2pHandle` abstraction boundary. The Dispatcher, Aggregator, engine, trigger, and submission subsystems are all unchanged. Inside the P2P module, four new components replace the libp2p Swarm: a network layer (`discovery::Network` or `lookup::Network`), a broadcast engine (`buffered::Engine`), a service router (new, application-level filtering), and an identity layer (Ed25519 derivation). All commonware types are kept strictly inside `p2p.rs` (or a `p2p/` module) to limit blast radius from future commonware ALPHA changes.

**Major components:**
1. `CommonwareP2pNetwork` — wraps `authenticated::discovery::Network` or `lookup::Network`; runs inside `commonware::tokio::Runner` on a dedicated OS thread; bridges commands via `mpsc::UnboundedSender<P2pCommand>`
2. `BroadcastEngine` — wraps `commonware_broadcast::buffered::Engine`; handles reliable broadcast, per-peer message caching, and digest-based catch-up retrieval; eliminates ~180 lines of custom catch-up and storage code
3. `ServiceRouter` — new thin struct with `HashSet<ServiceId>`; filters inbound messages from the single broadcast channel by subscribed services; replaces GossipSub topic isolation
4. `P2pHandle` (preserved) — unchanged external facade; `publish`, `subscribe`, `unsubscribe`, `get_status` interface stays identical; internal implementation changes from libp2p Swarm event loop to commonware coordinator task
5. `ed25519_signer_from_mnemonic()` — new identity function; derives Ed25519 key from BIP-39 mnemonic via ChaCha20Rng; replaces `keypair_from_mnemonic()` secp256k1 derivation

See `.planning/research/ARCHITECTURE.md` for data flow diagrams and the full build order.

### Critical Pitfalls

1. **Static channel model vs. dynamic service subscriptions** — Commonware channels cannot be created after `network.start()`. Avoid a 1:1 GossipSub-topic-to-channel mapping. Use a single broadcast channel for all services and filter by `service_id` in the new `ServiceRouter`. Decide this before writing any code.

2. **Commonware Runtime ownership conflict** — `commonware-runtime::tokio::Runner` creates its own Tokio runtime. Calling it inside WAVS's existing `tokio::spawn` will panic ("cannot start a runtime from within a runtime"). Run commonware's Runner on a dedicated `std::thread` and bridge via cross-thread channels. Prototype this first in Phase 2 before building any P2P logic on top.

3. **Catch-up protocol gap** — The custom `CatchUpRequest`/`CatchUpResponse` protocol provides service-scoped, TTL-bounded miss recovery. The `buffered::Engine` provides peer-scoped digest retrieval — close but not identical. Validate that the Engine's behavior meets the catch-up guarantee before removing custom catch-up code.

4. **Hard identity cutover** — secp256k1 peer IDs are mathematically unrelated to Ed25519 peer IDs derived from the same mnemonic. All operators must upgrade simultaneously. Bootstrap node addresses change. Plan coordinated upgrade and document clearly.

5. **Commonware ALPHA instability** — All commonware crates self-describe as ALPHA. Pin exact versions in `Cargo.toml`. Preserve the `P2pHandle` abstraction as an escape hatch. Do not let commonware types leak outside `p2p.rs`.

See `.planning/research/PITFALLS.md` for 12 total pitfalls with phase mappings.

## Implications for Roadmap

Based on research, the architecture's dependency chain naturally suggests 6 phases. Dependencies flow strictly top-down — each phase produces an artifact that the next phase builds on. The build order from ARCHITECTURE.md maps directly to phases.

### Phase 1: Cryptographic Identity

**Rationale:** Every subsequent component depends on having a working Ed25519 signer derived from the mnemonic. This is a pure function with no async, no networking, and no runtime concerns — lowest risk first.
**Delivers:** `ed25519_signer_from_mnemonic()` function with unit tests proving deterministic derivation from known mnemonics
**Addresses:** Key derivation (table stakes), Ed25519 seed choice (STACK.md recommendation: ChaCha20Rng seeded from BIP-39 entropy, not the insecure `from_seed(u64)`)
**Avoids:** Pitfall 4 (identity scheme change) — establishes the deterministic derivation scheme before anything depends on peer IDs

### Phase 2: P2P Network Skeleton

**Rationale:** Runtime integration is the highest-risk unknown in the entire migration. It must be validated before building broadcast, service routing, or any other P2P logic on top. A two-node integration test at this phase proves the runtime bridge works.
**Delivers:** `CommonwareP2pNetwork` wrapper with working peer connections; `commonware::tokio::Runner` running on a dedicated `std::thread`; two nodes can connect and the bridge channels pass messages
**Uses:** `commonware-p2p 2026.3.0`, `commonware-runtime 2026.3.0` (STACK.md); dedicated-thread integration pattern (STACK.md)
**Implements:** Connection layer component (ARCHITECTURE.md Phase 2)
**Avoids:** Pitfall 2 (runtime ownership conflict) — this phase's sole purpose is to prove the runtime strategy works

### Phase 3: Broadcast Integration

**Rationale:** With working peer connections, wire in the broadcast engine. This is where GossipSub and the entire custom catch-up protocol are replaced. Validates the single-channel routing decision.
**Delivers:** `BroadcastEngine` + `ServiceRouter` wired to `CommonwareP2pNetwork`; three-node test proves broadcast delivery and service-ID filtering; catch-up behavior validated against reconnect scenarios
**Uses:** `commonware-broadcast::buffered::Engine 2026.3.0` (STACK.md); `WavsMessage` with `Digestible + Codec` traits (ARCHITECTURE.md)
**Implements:** BroadcastEngine and ServiceRouter components (ARCHITECTURE.md Phase 3)
**Avoids:** Pitfall 1 (static channel vs. dynamic subscriptions) — single-channel approach is implemented here; Pitfall 3 (catch-up gap) — validate Engine behavior against catch-up requirements in this phase

### Phase 4: Full P2pHandle Reimplementation

**Rationale:** With the underlying network and broadcast working, implement the complete `P2pHandle` command surface. This is well-understood application-level logic with the lowest technical risk.
**Delivers:** Complete `P2pHandle` with `publish`, `subscribe`, `unsubscribe`, `get_status` reimplemented using commonware primitives; `ServiceRouter` subscribe/unsubscribe updates wired to `P2pCommand`; pending publish retry queue ported
**Implements:** P2pHandle facade preservation (ARCHITECTURE.md Phase 4)
**Avoids:** Pitfall 10 (pending publish retry queue) — explicit port of retry logic; Pitfall 8 (P2pStatus contract) — design backend-agnostic status struct here

### Phase 5: Config Migration

**Rationale:** Config is the last self-contained concern before full e2e validation. New `P2pConfig` format for commonware's discovery vs. lookup modes, updated `P2pStatus` struct, and dev-friendly local config preset.
**Delivers:** New `P2pConfig` (Disabled / Local / Remote) tailored to commonware; updated `P2pStatus` with socket addresses instead of multiaddrs; dev preset for localhost multi-operator testing; updated CLI `wait_for_p2p_ready()` compatibility
**Avoids:** Pitfall 7 (local dev discovery gap) — explicit dev config preset; Pitfall 8 (P2pStatus contract change) — backend-agnostic fields finalized here

### Phase 6: E2E Validation and libp2p Removal

**Rationale:** Full system validation before removing the old dependency. libp2p is removed only after `just test-wavs-e2e` passes, confirming the migration is complete and the old stack is no longer needed.
**Delivers:** Passing `just test-wavs-e2e` suite including `evm_multi_operator`; libp2p removed from `Cargo.toml`; all 13 libp2p feature flags gone; operator migration guide documenting bootstrap address format change and coordinated upgrade requirement
**Avoids:** Pitfall 4 (identity cutover) — migration guide and coordinated upgrade docs; Pitfall 9 (ALPHA instability) — exact version pinning confirmed; Pitfall 6 (rate limiting drops) — burst testing with `dev-tool send-triggers --count 1000`

### Phase Ordering Rationale

- Identity before networking: `ed25519_signer_from_mnemonic()` is a required input to `discovery::Network::new()`; no other phase can proceed without it
- Runtime prototype before broadcast: The dedicated-thread runtime strategy is unvalidated; all subsequent phases depend on it working correctly; discovering a fatal flaw in Phase 3 would require re-architecting Phase 2 work
- Broadcast before P2pHandle: The P2pHandle's `publish` command must route to a working broadcast engine; wiring an incomplete broadcast to the command interface would require rework
- Config after P2pHandle: Config parsing is straightforward once the components it configures are implemented; reversing this order would mean configuring components that do not yet exist
- libp2p removal last: Acts as the migration's "done" gate; ensures all functionality is working before deleting the fallback

### Research Flags

Phases likely needing deeper research during planning:

- **Phase 2 (P2P Network Skeleton):** Runtime integration is the most uncertain area. The dedicated-thread pattern is sound in theory but has no verified production examples in the WAVS context. May need to inspect commonware source for whether `Context` can be externally constructed. Also: Oracle dynamic peer set management (`track()` index semantics) needs validation.
- **Phase 3 (Broadcast Integration):** Catch-up guarantee equivalence needs verification. The `buffered::Engine` provides peer-scoped caching; the current protocol is service-scoped. Message size limits for `Submission` payloads need checking against commonware's configurable `max_message_size`.

Phases with standard patterns (skip research-phase):

- **Phase 1 (Cryptographic Identity):** Ed25519 from BIP-39 via ChaCha20Rng is a standard, well-documented pattern. STACK.md provides the exact code.
- **Phase 4 (Full P2pHandle):** Application-level command routing; no new external APIs needed.
- **Phase 5 (Config Migration):** Config struct refactoring; well-understood Rust pattern.
- **Phase 6 (E2E Validation):** Running the existing test suite; no new research needed.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All crates verified via docs.rs and GitHub. Version 2026.3.0 confirmed as latest release as of 2026-03-09. ALPHA warning is real but acceptable given abstraction boundary. |
| Features | HIGH | Feature table researched directly against official commonware docs and current WAVS source code. The static channel constraint is a documented API fact. |
| Architecture | MEDIUM | Component design is well-reasoned from docs. The dedicated-thread runtime integration pattern needs prototype validation — it is sound but unproven in this specific context. The open questions in ARCHITECTURE.md are real unknowns. |
| Pitfalls | MEDIUM | Critical pitfalls derived from direct code analysis of p2p.rs and commonware API surface. Catch-up equivalence (Pitfall 3) is the most uncertain assessment — depends on `buffered::Engine` behavior under reconnect scenarios that were inferred from docs, not tested. |

**Overall confidence:** MEDIUM

### Gaps to Address

- **Runtime integration prototype:** The `commonware::tokio::Runner` on a dedicated `std::thread` approach needs a minimal prototype before Phase 2 is scoped. If `Runner::start()` cannot be called from a non-async OS thread context, the entire integration strategy changes. Validate in Phase 2 task planning.

- **Oracle `track()` index semantics:** The Oracle manages peer sets at monotonically increasing `u64` indices. How this interacts with dynamic operator registration (operators joining/leaving between WAVS restarts) is not fully documented. Needs validation against the commonware chat example and GitHub issues during Phase 2.

- **Catch-up guarantee equivalence:** The `buffered::Engine`'s digest-based retrieval replaces the `CatchUpRequest`/`CatchUpResponse` protocol in theory. In practice, the guarantees differ: current catch-up is service-scoped and peer-targeted; the Engine is peer-scoped and broadcast-driven. Whether this meets the operational requirement (operators that restart during active quorum collection can catch up within 5 minutes) needs an explicit test added in Phase 3.

- **NAT traversal operational gap:** commonware has no AutoNAT equivalent. Production operators behind NAT must configure `dialable` address manually. This is acceptable for known-operator AVS deployments but needs clear documentation. Flag for post-migration operational review.

- **Message size budget:** `Submission` struct size with typical payload needs to be checked against commonware's `max_message_size` config (default unknown). Add to Phase 3 validation checklist.

## Sources

### Primary (HIGH confidence)
- [commonware-p2p docs.rs](https://docs.rs/commonware-p2p/2026.3.0/commonware_p2p/) — P2P API surface, channel registration, Oracle, discovery vs. lookup modes
- [commonware-broadcast docs.rs](https://docs.rs/commonware-broadcast/2026.3.0/commonware_broadcast/) — buffered Engine API, digest retrieval, message caching
- [commonware-cryptography docs.rs](https://docs.rs/commonware-cryptography/2026.3.0/commonware_cryptography/) — Ed25519 Signer trait, key generation
- [commonware-runtime docs.rs](https://docs.rs/commonware-runtime/2026.3.0/commonware_runtime/) — Runner::start(), Context creation, Tokio runner
- [commonware-runtime source (tokio/runtime.rs)](https://docs.rs/crate/commonware-runtime/2026.3.0/source/src/tokio/runtime.rs) — verified Runner creates own Tokio runtime
- [commonware GitHub monorepo](https://github.com/commonwarexyz/monorepo) — release history, CalVer scheme, v2026.3.0 as latest
- WAVS source: `packages/wavs/src/subsystems/aggregator/p2p.rs` (~1,840 lines) — current implementation, abstractions, catch-up protocol
- WAVS source: `packages/types/src/http.rs` — P2pStatus struct fields
- WAVS source: `packages/layer-tests/` — e2e test infrastructure and `wait_for_p2p_ready()`

### Secondary (MEDIUM confidence)
- [commonware-runtime blog post](https://commonware.xyz/blogs/commonware-runtime) — runtime design philosophy
- [commonware anti-framework blog post](https://commonware.xyz/blogs/commonware-the-anti-framework) — trait-based design rationale
- [commonware chat example](https://github.com/commonwarexyz/monorepo/blob/main/examples/chat/README.md) — reference implementation for channel registration pattern
- [Your P2P demo runs locally. Now what?](https://commonware.xyz/blogs/commonware-deployer) — deployment patterns

### Tertiary (LOW confidence)
- [Inside Commonware (Decipher Media)](https://medium.com/decipher-media/inside-commonware-50c58211953c) — third-party analysis; use STACK.md findings from official sources instead
- [commonware anti-framework philosophy (deepwiki)](https://deepwiki.com/commonwarexyz/monorepo/1.1-anti-framework-philosophy) — third-party wiki; corroborates official blog

---
*Research completed: 2026-03-17*
*Ready for roadmap: yes*
