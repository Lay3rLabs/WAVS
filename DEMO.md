# P2P Demo & Testing Guide

How to verify the commonware P2P migration works, from quick unit tests through
full multi-operator end-to-end.

---

## Quick Start

```bash
# Run all P2P unit and integration tests (no live stack needed, ~30s)
cargo test -p wavs -- p2p --test-threads=1

# Run the full E2E suite with multi-operator P2P
just test-wavs-e2e
```

---

## Test Layers

### 1. Unit Tests — Identity & Message Types

Tests in `packages/wavs/src/subsystems/aggregator/p2p.rs` (inline `#[cfg(test)]` module).
No network, no async runtime. Fast.

```bash
cargo test -p wavs -- p2p_broadcast_tests --test-threads=1
```

**What they cover:**

| Test | What it checks |
|------|----------------|
| `test_p2p_message_from_submission` | `P2pMessage::from_submission()` encodes service ID + payload correctly |
| `test_p2p_message_codec_roundtrip` | Encode → decode produces identical fields (Codec trait) |
| `test_p2p_message_digest_determinism` | Identical messages → same SHA-256 digest; different messages → different digest |
| `test_p2p_message_to_submission_roundtrip` | Full serialize/deserialize round-trip through `P2pMessage` |
| `test_service_router_empty_rejects_all` | Empty `ServiceRouter` rejects all messages |
| `test_service_router_subscribe_accept` | After `subscribe(A)`, accepts messages for A, rejects B |
| `test_service_router_unsubscribe` | After `unsubscribe(A)`, rejects messages for A again |
| `test_service_router_subscribed_services` | `subscribed_services()` returns correct list |
| `test_retry_queue_empty` | Empty queue returns empty drain |
| `test_retry_queue_push_drain_fifo` | Messages drained in FIFO order |
| `test_retry_queue_overflow_drops_oldest` | Queue at capacity drops oldest on push (bounded at 64) |
| `test_retry_queue_drain_empty` | Second drain of empty queue returns empty |

---

### 2. Integration Tests — Identity (no network)

Tests in `packages/wavs/tests/p2p_identity_tests.rs`.

```bash
cargo test -p wavs --test p2p_identity_tests -- --nocapture
```

**What they cover:**

| Test | What it checks |
|------|----------------|
| `test_deterministic_derivation` | Same mnemonic always produces the same Ed25519 keypair |
| `test_consistent_across_restarts` | `ed25519_signer_from_mnemonic()` is stable across calls |
| `test_different_mnemonics_produce_different_keys` | Different mnemonics → different peer IDs |
| `test_invalid_mnemonic_returns_error` | Bad mnemonic returns an error, does not panic |
| `test_p2p_config_default_is_disabled` | `P2pConfig::default()` is `Disabled` |
| `test_p2p_config_local_deserialization` | `[wavs.p2p.local]` TOML deserializes correctly |
| `test_p2p_config_remote_deserialization` | `[wavs.p2p.remote]` TOML deserializes correctly |
| `test_p2p_config_disabled_has_no_port` | `P2pConfig::Disabled` has no listen port |

---

### 3. Integration Tests — Connectivity (real sockets, localhost)

Tests in `packages/wavs/tests/p2p_connectivity_tests.rs`.
Spin up real commonware runtimes on localhost. Ports start at `19000`.

```bash
cargo test -p wavs --test p2p_connectivity_tests -- --test-threads=1 --nocapture
```

> `--test-threads=1` is required — tests bind to localhost ports and conflict if run in parallel.

**What they cover:**

| Test | What it checks |
|------|----------------|
| `test_lookup_mode_two_nodes_connect` | Two nodes in Local mode connect to each other via known addresses |
| `test_unauthorized_peer_rejected` | Node with pubkey not in `authorized_peers` cannot connect |
| `test_discovery_mode_two_nodes` | Two nodes in Remote mode discover each other via bootstrapper |
| `test_block_peer` | `block_peer()` disconnects a peer and prevents reconnection |
| `test_auto_reconnect` | Node survives bootstrapper unavailability; `dial_frequency` retries automatically |

---

### 4. Integration Tests — Broadcast (real P2P, real messages)

Tests in `packages/wavs/tests/p2p_broadcast_tests.rs`.
Spin up 2–3 real commonware nodes and exchange actual `Submission` payloads.

```bash
cargo test -p wavs --test p2p_broadcast_tests -- --test-threads=1 --nocapture
```

**What they cover:**

| Test | What it checks |
|------|----------------|
| `test_broadcast_to_all_peers` | Published submission arrives at all connected peers |
| `test_service_filtering` | Operator subscribed to service A does not receive service B messages |
| `test_p2p_handle_api_preserved` | `publish`, `subscribe`, `unsubscribe`, `get_status` all work; Aggregator interface unchanged |
| `test_retry_queue_on_no_peers` | Messages queued when no peers; delivered after peer connects |
| `test_deduplication_by_digest` | Same message broadcast twice arrives exactly once |
| `test_catchup_after_reconnect` | Node that reconnects receives messages broadcast while it was away |
| `test_cache_bounded_deque_size` | Catch-up buffer respects `deque_size` limit |
| `test_status_connected_peers_after_broadcast` | `/p2p/status` returns correct `connected_peers` count and hex peer IDs after broadcast |

---

### 5. E2E Tests — Full Multi-Operator Stack

Requires all services running. Starts real WAVS nodes, Anvil chain, and middleware.

```bash
just test-wavs-e2e
```

The test suite uses `p2p = "remote"` (discovery mode) by default. To isolate the
multi-operator test:

```toml
# packages/layer-tests/layer-tests.toml
mode = { "isolated" = [{ evm = "multi_operator" }] }
```

Then:

```bash
just test-wavs-e2e
```

To switch to Local (lookup) mode:

```toml
# packages/layer-tests/layer-tests.toml
p2p = "local"
```

---

## Manual Two-Node Demo

Verify the P2P layer works with two live WAVS nodes on localhost.

### Step 1 — Get peer IDs

```bash
# Node 1 (use any BIP-39 mnemonic)
wavs-cli p2p identity --mnemonic "test test test test test test test test test test test junk"
# → e.g. a1b2c3d4e5f6...  (64 hex chars)

# Node 2 (different mnemonic)
wavs-cli p2p identity --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
# → e.g. b2c3d4e5f6a1...
```

### Step 2 — Write configs

**`/tmp/wavs-node-1.toml`:**
```toml
[wavs]
port = 8041
signing_mnemonic = "test test test test test test test test test test test junk"

[wavs.p2p.local]
listen_port = 9000
peer_addresses = ["<NODE2_PUBKEY>@127.0.0.1:9001"]
authorized_peers = ["<NODE2_PUBKEY>"]
```

**`/tmp/wavs-node-2.toml`:**
```toml
[wavs]
port = 8042
signing_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

[wavs.p2p.local]
listen_port = 9001
peer_addresses = ["<NODE1_PUBKEY>@127.0.0.1:9000"]
authorized_peers = ["<NODE1_PUBKEY>"]
```

Replace `<NODE1_PUBKEY>` and `<NODE2_PUBKEY>` with the hex strings from Step 1.

### Step 3 — Start nodes

```bash
# Terminal 1
WAVS_SIGNING_MNEMONIC="test test test test test test test test test test test junk" \
  wavs --home /tmp/wavs-node-1.toml

# Terminal 2
WAVS_SIGNING_MNEMONIC="abandon abandon abandon..." \
  wavs --home /tmp/wavs-node-2.toml
```

### Step 4 — Verify connectivity

```bash
# Node 1 status
curl -s http://127.0.0.1:8041/p2p/status | jq .

# Node 2 status
curl -s http://127.0.0.1:8042/p2p/status | jq .
```

Expected on each node once connected:
```json
{
  "enabled": true,
  "local_peer_id": "<hex>",
  "listen_addresses": ["0.0.0.0:900X"],
  "connected_peers": 1,
  "peer_ids": ["<other_node_hex>"],
  "subscribed_services": []
}
```

`connected_peers` starts at `0` and increments to `1` after the first broadcast.

---

## What to Look For in Logs

```
INFO wavs::p2p: P2P lookup network: listening on 0.0.0.0:9000, 1 peers, 1 authorized
INFO wavs::p2p: Broadcast delivered to 1 peers
INFO wavs::p2p: Inbound P2P message for service <id>, forwarding to aggregator
```

With `RUST_LOG=info,wavs=debug`:
```
DEBUG wavs::p2p: Subscribed to service: <service_id>
DEBUG wavs::p2p: Filtered message for unsubscribed service
DEBUG wavs::p2p: Duplicate message filtered by digest
```

---

## Troubleshooting

**`connected_peers` stays at 0**
- Check that `authorized_peers` in each node's config lists the *other* node's pubkey
- Check that `peer_addresses` uses the correct port
- Connectivity only updates after the first successful broadcast (the tracker is populated from ack recipients and inbound senders)

**`parse_bootstrapper` error in logs**
- Remote mode bootstrapper addresses must be `<hex_pubkey>@<host>:<port>`, not bare `host:port`
- Use `wavs-cli p2p identity` to get the hex pubkey, then compose the address

**Port conflicts in tests**
- Run P2P integration tests with `--test-threads=1`; parallel runs collide on localhost ports
- Tests use ports starting at `19000` + offset

**Runtime drop panic after tests**
- Cosmetic. The commonware runtime's internal Tokio runtime is dropped on thread exit after the test framework tears down. All assertions pass before the panic.
