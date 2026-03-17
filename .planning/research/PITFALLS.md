# Domain Pitfalls

**Domain:** P2P networking migration (libp2p to commonware) in WAVS aggregator
**Researched:** 2026-03-17

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: Commonware's Fixed Peer Set Model vs. WAVS Dynamic Service Subscriptions

**What goes wrong:** Commonware-p2p is designed around a "fixed set of authenticated peers" managed through an `Oracle` that tracks peer sets at sequential `u64` indices. WAVS currently uses dynamic GossipSub topic subscriptions -- operators subscribe/unsubscribe to per-service topics at runtime (`SubscribeService`/`UnsubscribeService` commands). Attempting to map GossipSub's per-topic pub/sub directly onto commonware's channel model will create a fundamental architecture mismatch.

**Why it happens:** In libp2p GossipSub, any peer can subscribe to any topic at any time, and message routing is topic-scoped. In commonware, channels are registered at network initialization time with `network.register(channel_id, quota, backlog)`, and the `Oracle.track()` manages which peers are authorized. There is no runtime equivalent to "subscribing to a new topic" -- channels are pre-registered and peer sets are updated via the Oracle.

**Consequences:** If the migration attempts a 1:1 mapping (one GossipSub topic = one commonware channel), it hits a wall: you cannot dynamically create channels after `network.start()`. If it uses a single channel for all services, it loses per-service message isolation, meaning every operator receives every service's messages and must filter locally -- wasting bandwidth and breaking the current scoping model. Operators running 1 service would receive traffic for all 50 services on the network.

**Prevention:**
- Design the channel strategy early. Two viable approaches:
  1. **Single broadcast channel + application-level filtering**: Use one commonware-broadcast `buffered::Engine` for all submissions. The `Submission` struct already contains `service_id` -- receivers filter by services they care about. Simple but loses network-level isolation.
  2. **Pre-allocated channel pool**: Register N channels at startup (e.g., channels 0-255), hash `service_id` to a channel index. Provides partial isolation. Risk: channel collisions if many services hash to the same channel.
- The `Oracle.track()` peer set should be the full set of known WAVS operators, not per-service subsets. Service-level filtering happens at the application layer.
- Validate the chosen approach against the e2e multi-operator test (`evm_multi_operator`) early.

**Detection:** You will notice this immediately when trying to implement `P2pCommand::Subscribe` -- there is no commonware equivalent. If the implementer is writing a `HashMap<ServiceId, ChannelId>` that grows at runtime, they have hit this pitfall.

**Phase mapping:** Must be resolved in Phase 1 (architecture design) before any code is written.

### Pitfall 2: Commonware-Runtime Ownership vs. Existing Tokio Runtime

**What goes wrong:** Commonware has its own runtime abstraction (`commonware-runtime`) that wraps Tokio for production use. The `commonware-runtime::tokio` module creates and owns its own Tokio runtime via the `Runner` trait. WAVS already has its own Tokio runtime managing the dispatcher, engine, trigger manager, HTTP server, and all other subsystems. Attempting to run commonware's network inside WAVS's existing runtime -- or letting commonware create a second runtime -- leads to runtime conflicts, deadlocks, or panics.

**Why it happens:** `commonware-runtime::tokio::Executor` implements `Runner::start()` which blocks on the provided future. It expects to be the top-level runtime controller. WAVS spawns the P2P event loop via `tokio::spawn()` inside the dispatcher's existing runtime. These two runtime ownership models conflict.

**Consequences:** Double-runtime scenarios (two Tokio runtimes in the same process) cause `tokio::spawn` to panic if called from outside the expected runtime context. Blocking on `Runner::start()` from within an existing async context causes the current task to block forever. Attempting to pass Tokio handles across runtime boundaries causes subtle Send/Sync issues.

**Prevention:**
- Investigate whether commonware-p2p can accept an externally-provided runtime context rather than creating its own. The docs state "developers can implement the exported traits from commonware-runtime and drop in their runtime to any of the Commonware Library primitives" -- this is the path.
- Write a thin adapter that implements commonware-runtime's `Spawner`, `Clock`, and other traits by delegating to WAVS's existing Tokio runtime handle.
- Alternatively, run commonware in a dedicated `std::thread` with its own Tokio runtime (isolated), communicating with the main WAVS runtime via channels. This is simpler but adds latency.
- Prototype the runtime integration first before building any P2P logic on top.

**Detection:** Panics at startup with "cannot start a runtime from within a runtime" or "no reactor is running". Deadlocks where the P2P network never emits events.

**Phase mapping:** Must be resolved in Phase 1 alongside the channel architecture. This is a blocking prerequisite.

### Pitfall 3: Losing the Catch-Up Protocol Without a Replacement

**What goes wrong:** The current libp2p implementation has a purpose-built catch-up protocol using Request/Response: when a peer reconnects, it sends a `CatchUpRequest` for each subscribed service and receives a `CatchUpResponse` with recent submissions. This ensures operators that were briefly offline do not miss quorum-critical submissions. The migration drops this without realizing commonware-broadcast's `buffered::Engine` does not fully replicate this behavior.

**Why it happens:** Commonware-broadcast's buffered engine caches messages "per peer" with bounded queues, and supports digest-based retrieval. But it is a broadcast primitive, not a targeted request/response protocol. The current catch-up protocol is service-scoped, peer-targeted, and bounded by configurable limits (`max_catchup_submissions`, `max_concurrent_catchup_requests_per_service`). The buffered engine's caching is peer-scoped, not service-scoped.

**Consequences:** Operators that go offline briefly (network blip, restart) miss submissions that were broadcast while they were disconnected. Without catch-up, they never reach quorum for events that occurred during the gap. The aggregator's retry mechanism can partially compensate (other operators re-send on new events), but there is a window where quorum is permanently lost for specific events.

**Prevention:**
- Map the exact catch-up guarantees needed:
  - After reconnection, an operator must be able to receive submissions broadcast in the last N minutes (currently configurable via `submission_ttl_secs`, default 5 minutes).
  - Catch-up must be bounded to prevent DoS (current `MAX_RESPONSE_SIZE` is 10MB).
- Evaluate whether commonware-broadcast's `buffered::Engine` + digest-based retrieval can be configured to provide equivalent guarantees. The "request a message by digest" capability might work if digests are known.
- If the buffered engine is insufficient, implement a separate catch-up mechanism using a dedicated commonware channel (direct peer-to-peer request/response over the authenticated connection).
- The `stored_submissions` HashMap and TTL-based cleanup in `EventLoopState` will need equivalents regardless of approach.

**Detection:** Multi-operator e2e tests pass when all operators start simultaneously, but fail intermittently when operators restart or have network partitions. The `evm_multi_operator` test may mask this because all operators start together in tests.

**Phase mapping:** Phase 2 (implementation) -- but the approach must be decided in Phase 1.

### Pitfall 4: Identity Scheme Change Breaking Operator Continuity

**What goes wrong:** The migration changes P2P identity from secp256k1 (derived from EVM signing mnemonic at HD path m/44'/60'/0'/0/0) to Ed25519 (commonware-cryptography). This means every operator gets a new peer ID. Existing operators cannot be recognized by their old peer IDs, and any infrastructure that tracks peer IDs (monitoring, logging, whitelists, firewall rules) breaks.

**Why it happens:** The `keypair_from_mnemonic()` function currently derives a secp256k1 keypair from the signing mnemonic and converts it to a libp2p `Keypair`. In commonware, identity is Ed25519-based. You cannot derive the same peer ID from the same mnemonic because the curves are different. The public keys are mathematically unrelated even when derived from the same seed material.

**Consequences:**
- All operator peer IDs change simultaneously on upgrade. No rolling upgrade is possible at the P2P layer -- it is a hard cutover.
- Any operator that upgrades while others have not will be unable to communicate with non-upgraded peers (different P2P protocol entirely).
- Bootstrap node addresses change (they embed peer ID). All operators must update their bootstrap configuration simultaneously.
- Monitoring dashboards tracking peer IDs need updating.
- The `/p2p/status` endpoint returns different-format peer IDs.

**Prevention:**
- Accept this is a breaking change and plan for it. The PROJECT.md already identifies "clean break on P2P config format" as a key decision.
- Coordinate the upgrade: all operators in a deployment must upgrade simultaneously. Document this clearly.
- The Ed25519 key derivation from the mnemonic must be deterministic and documented. Use a different HD path or derivation scheme to avoid confusion with the EVM secp256k1 keys.
- Consider deriving Ed25519 from the mnemonic using a different namespace (e.g., HKDF with domain separator "wavs-p2p-ed25519") to prevent cross-protocol key reuse.
- Update the `peer_id_from_mnemonic()` utility function to use the new scheme.

**Detection:** Operators log "unknown peer" or "connection rejected" errors immediately after partial upgrade. The `/p2p/status` endpoint shows 0 connected peers on upgraded nodes.

**Phase mapping:** Phase 1 (key derivation design) and Phase 3 (operator documentation, blog post).

## Moderate Pitfalls

### Pitfall 5: Message Serialization Format Incompatibility

**What goes wrong:** The current implementation serializes `Submission` as JSON via serde for both GossipSub messages and catch-up protocol. Commonware channels send raw bytes (`IoBuf`). If the serialization format changes or additional framing is added without careful consideration, old submissions cached in operator storage become unparseable after upgrade.

**Prevention:**
- Continue using serde JSON for the `Submission` payload bytes. Commonware channels are byte-agnostic.
- Define a clear message envelope: version byte + JSON payload. This allows future format migrations.
- The existing `Submission` struct serialization is the contract -- do not change it during the P2P migration.

**Phase mapping:** Phase 2 (implementation).

### Pitfall 6: Rate Limiting Behavioral Differences

**What goes wrong:** Commonware-p2p enforces per-channel rate limits via `Quota` at the network layer. GossipSub has no per-topic rate limiting -- it relies on mesh parameters and message deduplication. After migration, legitimate bursts of submissions (e.g., many events triggering simultaneously) may be silently dropped by commonware's rate limiter, causing missed quorum.

**Prevention:**
- Analyze peak submission rates from production or test logs. The current `DEFAULT_MAX_PENDING_PUBLISHES` is 1000, suggesting bursts are expected.
- Set commonware channel quotas generously (e.g., `Quota::per_second(1000)`) during initial migration, then tune based on observed traffic.
- Commonware's `CheckedSender` returns errors when rate-limited. Implement a retry queue similar to the current `pending_publishes` VecDeque.
- Test with the `dev-tool send-triggers --count 1000` command to verify burst handling.

**Phase mapping:** Phase 2 (implementation) and Phase 4 (performance tuning).

### Pitfall 7: Discovery Model Mismatch for Local Development

**What goes wrong:** The current system has two discovery modes: mDNS (automatic local discovery) and Kademlia DHT (bootstrap-based remote discovery). Commonware only offers bootstrapper-based discovery (the `discovery` module) or address-known lookup (the `lookup` module). There is no mDNS equivalent. Local development becomes harder because developers must manually configure bootstrapper addresses even for single-machine testing.

**Prevention:**
- For local dev, use `Config::local()` which is designed for testing scenarios with known addresses.
- Alternatively, use the `lookup` module where all peer addresses are known upfront (hardcoded localhost addresses).
- Create a dev-friendly config preset that auto-configures localhost addresses for N operators on sequential ports. The existing test infrastructure (`DEFAULT_P2P_BASE_PORT = 9000`) provides the pattern.
- Document the local dev setup clearly. Operators accustomed to "just works" mDNS will need explicit instructions.

**Phase mapping:** Phase 2 (implementation) and Phase 3 (documentation).

### Pitfall 8: P2pStatus Endpoint Contract Change

**What goes wrong:** The `P2pStatus` struct (in `packages/types/src/http.rs`) exposes libp2p-specific concepts: `listen_addresses` (multiaddr format), `external_addresses` (AutoNAT-discovered), `subscribed_topics`, `topic_peer_counts`. Commonware has different addressing (socket addresses, not multiaddrs), no AutoNAT, and channels instead of topics. External tooling, the CLI (`wait_for_p2p_ready`), and the Tauri desktop app all consume this struct.

**Prevention:**
- Design the new `P2pStatus` struct before implementation. Map each existing field:
  - `local_peer_id` -> Ed25519 public key string (format changes)
  - `listen_addresses` -> socket address format (not multiaddr)
  - `external_addresses` -> may not exist (commonware may not have NAT traversal)
  - `subscribed_topics` -> registered channels or service IDs
  - `topic_peer_counts` -> channel-level peer counts (if available from Oracle)
  - `connected_peers` + `peer_ids` -> should still be available from the network
- The CLI `wait_for_p2p_ready()` checks `connected_peers >= min_peers`. This must continue to work. The e2e test framework depends on it.
- Consider making `P2pStatus` backend-agnostic: remove multiaddr/topic terminology, use generic "addresses" and "channels".

**Phase mapping:** Phase 1 (API design) and Phase 2 (implementation).

### Pitfall 9: Commonware ALPHA Stability Risk

**What goes wrong:** Commonware-p2p is explicitly marked as "ALPHA software and is not yet recommended for production use." ALPHA stability in the commonware library means "breaking changes are expected with no migration path provided." Building production infrastructure on an ALPHA dependency means any commonware release could break WAVS's P2P layer with no upgrade path.

**Prevention:**
- Pin the exact commonware crate versions in `Cargo.toml`. Do not use `^` or `~` version ranges.
- Vendor or fork the commonware crates if stability is critical. The MIT/Apache-2.0 license permits this.
- Monitor the commonware GitHub releases and changelog proactively.
- Abstract the P2P layer behind a clean trait boundary (`P2pHandle` already exists) so that swapping back to libp2p or another library is feasible if commonware ALPHA breaks in an unrecoverable way.
- The current `P2pHandle` abstraction (publish, subscribe, unsubscribe, get_status) is already a good abstraction layer -- preserve it.

**Phase mapping:** Ongoing concern across all phases. Pin versions in Phase 2.

## Minor Pitfalls

### Pitfall 10: Pending Publish Retry Queue Reimplementation

**What goes wrong:** The current implementation has a carefully tuned retry queue for failed publishes (`PendingPublish` struct, `retry_pending_publishes()`, configurable `max_retry_duration_secs`, `retry_interval_ms`, `max_pending_publishes`). This handles the common case where a publish fails because no peers are subscribed to the topic yet (mesh still forming). Forgetting to reimplement this in commonware leads to silent message loss during startup or network churn.

**Prevention:**
- Port the retry queue logic. Commonware's `CheckedSender` and `LimitedSender` traits provide error feedback -- use it to drive retries.
- Preserve the existing configurable parameters (max retry duration, interval, max queue size).
- Test with staggered operator startup to verify retry behavior.

**Phase mapping:** Phase 2 (implementation).

### Pitfall 11: Deduplication Logic Differences

**What goes wrong:** GossipSub has built-in message deduplication via the `message_id_fn` (hash of data + source + topic). Commonware-broadcast may handle deduplication differently or not at all at the network layer. The application-level deduplication in `store_submission()` (by signer address + event ID) is a second layer. Removing network-level dedup could increase bandwidth usage and processing overhead.

**Prevention:**
- Verify whether commonware-broadcast's buffered engine deduplicates. If not, the existing application-level deduplication (`store_submission()`) is sufficient for correctness but bandwidth increases.
- Consider adding a `HashSet<MessageId>` at the application layer if commonware does not deduplicate.

**Phase mapping:** Phase 2 (implementation).

### Pitfall 12: NAT Traversal Gap

**What goes wrong:** The current libp2p stack includes AutoNAT + Identify for discovering external addresses behind NAT. Commonware-p2p does not appear to include NAT traversal capabilities. Operators behind NAT cannot receive inbound connections without additional infrastructure.

**Prevention:**
- Document that operators need to configure proper port forwarding or use a public IP. This is already the case for production deployments.
- The `Config` has a `dialable` socket address field that operators must set to their externally-reachable address.
- For development, this is a non-issue (localhost).
- Consider whether a relay service or VPN is needed for NAT'd production deployments. This is out of scope for the initial migration but should be flagged.

**Phase mapping:** Phase 3 (documentation) and post-migration operational consideration.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Architecture design | Channel model mismatch (Pitfall 1) | Decide single-channel vs pool before coding. Prototype with the chat example. |
| Architecture design | Runtime ownership conflict (Pitfall 2) | Build runtime adapter or isolated thread first. |
| Architecture design | Catch-up strategy (Pitfall 3) | Evaluate buffered engine capabilities against requirements. |
| Key derivation | Identity scheme change (Pitfall 4) | Define deterministic Ed25519 derivation from mnemonic. Document breaking change. |
| Implementation | Rate limiting drops (Pitfall 6) | Set generous quotas, implement retry queue. |
| Implementation | Discovery for local dev (Pitfall 7) | Build dev-friendly config presets. |
| API surface | P2pStatus contract (Pitfall 8) | Design backend-agnostic status struct. Update CLI and tests. |
| Testing | Catch-up regression (Pitfall 3) | Add test for operator restart during active submissions. |
| Testing | Deduplication changes (Pitfall 11) | Monitor bandwidth in multi-operator tests. |
| Documentation | Operator migration (Pitfall 4) | Coordinated upgrade guide, new bootstrap instructions. |
| Ongoing | ALPHA instability (Pitfall 9) | Pin versions, maintain P2pHandle abstraction as escape hatch. |

## Sources

- [commonware-p2p docs.rs](https://docs.rs/commonware-p2p/latest/commonware_p2p/) - MEDIUM confidence (official docs)
- [commonware-p2p authenticated::discovery](https://docs.rs/commonware-p2p/latest/commonware_p2p/authenticated/discovery/index.html) - MEDIUM confidence (official docs)
- [commonware-broadcast docs.rs](https://docs.rs/commonware-broadcast/latest/commonware_broadcast/) - MEDIUM confidence (official docs)
- [commonware-runtime docs.rs](https://docs.rs/commonware-runtime/latest/commonware_runtime/) - MEDIUM confidence (official docs)
- [commonware GitHub monorepo](https://github.com/commonwarexyz/monorepo) - HIGH confidence (primary source)
- [commonware chat example](https://github.com/commonwarexyz/monorepo/blob/main/examples/chat/README.md) - HIGH confidence (reference implementation)
- [commonware-runtime blog post](https://commonware.xyz/blogs/commonware-runtime) - MEDIUM confidence (official blog)
- [Inside Commonware (Decipher Media)](https://medium.com/decipher-media/inside-commonware-50c58211953c) - LOW confidence (third-party analysis)
- WAVS codebase analysis of `packages/wavs/src/subsystems/aggregator/p2p.rs` (~1,800 lines) - HIGH confidence (direct code inspection)
- WAVS codebase analysis of `packages/types/src/http.rs` P2pStatus struct - HIGH confidence (direct code inspection)
- WAVS e2e test infrastructure in `packages/layer-tests/` - HIGH confidence (direct code inspection)
