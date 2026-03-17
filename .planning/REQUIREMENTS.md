# Requirements: WAVS Commonware P2P Migration

**Defined:** 2026-03-17
**Core Value:** Multi-operator signature aggregation over P2P must work reliably using commonware instead of libp2p

## v1 Requirements

### Identity

- [ ] **IDEN-01**: P2P identity derived deterministically from `WAVS_SIGNING_MNEMONIC` as Ed25519 keypair via ChaCha20Rng
- [ ] **IDEN-02**: Peer ID is consistent across node restarts with same mnemonic

### Networking

- [ ] **NET-01**: Operators discover peers via commonware-p2p discovery mode with bootstrappers (production)
- [ ] **NET-02**: Operators connect to peers via commonware-p2p lookup mode with known addresses (local dev)
- [ ] **NET-03**: Peer connections are encrypted and authenticated by Ed25519 identity
- [ ] **NET-04**: Node reconnects to bootstrappers automatically when peers are lost

### Broadcast

- [ ] **BCAST-01**: Operator can broadcast signed submission to all connected peers
- [ ] **BCAST-02**: Messages are deduplicated by cryptographic digest
- [ ] **BCAST-03**: Submission type implements commonware Codec and Digestible traits
- [ ] **BCAST-04**: Failed publishes (no peers) are retried with bounded queue
- [ ] **BCAST-05**: Per-service message isolation via application-level service_id filtering on single channel

### Catch-Up

- [ ] **CATCH-01**: Reconnecting peer retrieves missed submissions via buffered Engine digest-based caching
- [ ] **CATCH-02**: Message storage is bounded per peer (configurable deque_size)

### Security

- [ ] **SEC-01**: Oracle-based peer set management authorizes only known operators
- [ ] **SEC-02**: Built-in per-peer and per-subnet rate limiting active on all connections
- [ ] **SEC-03**: Misbehaving peers can be blocked by cryptographic identity

### Config

- [ ] **CFG-01**: New P2P config format in wavs.toml (Disabled / Local / Remote) tailored to commonware
- [ ] **CFG-02**: Configurable listen port, bootstrappers, timeouts, deque sizes
- [ ] **CFG-03**: Local dev preset with localhost peer addresses for multi-operator testing

### Observability

- [ ] **OBS-01**: `/p2p/status` endpoint returns peer ID, listen addresses, connected peers, subscribed services
- [ ] **OBS-02**: Status uses socket addresses (not multiaddr) and Ed25519 public keys

### Integration

- [ ] **INT-01**: P2pHandle API (publish, subscribe, unsubscribe, get_status) preserved — Aggregator sees no changes
- [ ] **INT-02**: All existing e2e tests pass (`just test-wavs-e2e`)
- [ ] **INT-03**: libp2p dependency removed from Cargo.toml

### Documentation

- [ ] **DOC-01**: `docs/P2P.md` updated with commonware setup, config examples, multi-node instructions
- [ ] **DOC-02**: Blog post in `docs/blog/` announcing commonware integration (announcement style)
- [ ] **DOC-03**: Operator migration guide documenting identity change, config format change, coordinated upgrade requirement

## v2 Requirements

### Advanced Security

- **SEC-04**: On-chain operator registry integration with Oracle (auto-authorize registered operators)
- **SEC-05**: Namespace-scoped replay protection across dev/prod networks

### Testing

- **TEST-01**: Simulated networking tests using commonware-p2p::simulated for deterministic P2P unit tests
- **TEST-02**: Priority message support for quorum-critical submissions

### Operations

- **OPS-01**: NAT traversal guidance or infrastructure for operators behind NAT

## Out of Scope

| Feature | Reason |
|---------|--------|
| Custom NAT traversal (AutoNAT replacement) | Commonware has no equivalent; production operators have public IPs |
| mDNS zero-config discovery | Replaced by lookup mode with known addresses; acceptable DX change |
| Backward compatibility with libp2p peers | Clean break — all operators upgrade simultaneously |
| Custom request/response catch-up protocol | Replaced by commonware-broadcast buffered Engine |
| GossipSub mesh parameter tuning | GossipSub-specific concepts don't exist in commonware |
| Multiaddr format preservation | Switched to socket addresses + Ed25519 public keys |
| Dynamic channel registration workarounds | Single channel with service filtering is simpler and correct |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| IDEN-01 | Phase 1 | Pending |
| IDEN-02 | Phase 1 | Pending |
| NET-01 | Phase 1 | Pending |
| NET-02 | Phase 1 | Pending |
| NET-03 | Phase 1 | Pending |
| NET-04 | Phase 1 | Pending |
| SEC-01 | Phase 1 | Pending |
| SEC-02 | Phase 1 | Pending |
| SEC-03 | Phase 1 | Pending |
| BCAST-01 | Phase 2 | Pending |
| BCAST-02 | Phase 2 | Pending |
| BCAST-03 | Phase 2 | Pending |
| BCAST-04 | Phase 2 | Pending |
| BCAST-05 | Phase 2 | Pending |
| CATCH-01 | Phase 2 | Pending |
| CATCH-02 | Phase 2 | Pending |
| INT-01 | Phase 2 | Pending |
| CFG-01 | Phase 3 | Pending |
| CFG-02 | Phase 3 | Pending |
| CFG-03 | Phase 3 | Pending |
| OBS-01 | Phase 3 | Pending |
| OBS-02 | Phase 3 | Pending |
| INT-02 | Phase 4 | Pending |
| INT-03 | Phase 4 | Pending |
| DOC-01 | Phase 4 | Pending |
| DOC-02 | Phase 4 | Pending |
| DOC-03 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 27 total
- Mapped to phases: 27
- Unmapped: 0

---
*Requirements defined: 2026-03-17*
*Last updated: 2026-03-17 after roadmap creation (consolidated from 5 phases to 4)*
