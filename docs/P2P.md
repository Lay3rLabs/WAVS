# P2P Networking

```
┌─────────────────────────────────────────────────────────────────────┐
│                        WAVS Operator Node                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │
│  │   Trigger    │───▶│   Engine     │───▶│   Submission Manager │  │
│  │   Manager    │    │  (WASM exec) │    │                      │  │
│  └──────────────┘    └──────────────┘    └──────────┬───────────┘  │
│                                                      │              │
│                                          ┌───────────▼───────────┐  │
│                                          │     Aggregator        │  │
│                                          │                       │  │
│                                          │  ┌─────────────────┐  │  │
│                                          │  │  Quorum Queue   │  │  │
│                                          │  │  (per eventId)  │  │  │
│                                          │  └────────┬────────┘  │  │
│                                          │           │           │  │
│                                          │  ┌────────▼────────┐  │  │
│  ┌──────────────────────────────────┐    │  │   P2P Network   │  │  │
│  │      Other WAVS Nodes            │◀───┼──│  commonware-p2p │  │  │
│  │  (receive/send signatures)       │───▶│  │ Broadcast Engine│  │  │
│  └──────────────────────────────────┘    │  └────────┬────────┘  │  │
│                                          │           │           │  │
│                                          │  ┌────────▼────────┐  │  │
│                                          │  │    Submit       │  │  │
│                                          │  │  (on quorum)    │  │  │
│                                          │  └────────┬────────┘  │  │
│                                          └───────────┼───────────┘  │
│                                                      │              │
└──────────────────────────────────────────────────────┼──────────────┘
                                                       │
                                                       ▼
                                              ┌────────────────┐
                                              │   Blockchain   │
                                              └────────────────┘
```

---

## Overview

P2P networking is an optional feature of the Aggregator subsystem. When enabled,
operators broadcast signed submissions to peers via a commonware broadcast channel,
and the Aggregator collects signatures to reach quorum before posting on-chain.

Uses commonware-p2p for authenticated peer networking with Ed25519 identities.
A single broadcast channel handles all services, with application-level filtering
by service ID (ServiceRouter).

---

## Architecture

- **Identity**: Ed25519 keypair derived from `WAVS_SIGNING_MNEMONIC` via ChaCha20Rng (BIP-39 seed). Deterministic: same mnemonic always produces the same peer ID.
- **Two modes**: Local (lookup with known peer addresses) and Remote (discovery with bootstrappers).
- **Broadcast**: Single commonware broadcast channel with ServiceRouter filtering by service ID. Two channels are registered per network — Channel 0 for the Broadcast Engine (caching/catch-up), Channel 1 for real-time forwarding to the Aggregator.
- **Catch-up**: Buffered Broadcast Engine caches messages per peer. On reconnection, the Engine replays cached messages to the reconnecting peer.
- **Security**: Oracle-based peer authorization — only peers whose Ed25519 public keys appear in `authorized_peers` can connect. Built-in per-peer rate limiting via commonware's default `Config::local()` settings.

---

## Configuration

P2P is disabled by default. Add a `[wavs.p2p]` block to `wavs.toml` to enable.

### Disabled (default)

Omit the `[wavs.p2p]` block entirely, or explicitly set:

```toml
# p2p = "disabled"
```

The node operates in single-operator mode: no P2P networking, no broadcast, no bootstrapping.

### Local — Lookup Mode (development / testing)

```toml
[wavs.p2p.local]
listen_port = 9000
peer_addresses = ["<hex_ed25519_pubkey>@127.0.0.1:9001"]
authorized_peers = ["<hex_ed25519_pubkey>"]
max_message_size = 65536    # Max P2P message size in bytes (default: 64 KB)
deque_size = 128            # Broadcast cache per peer for catch-up (default: 128 messages)
```

Lookup mode uses explicit peer addresses — no automatic LAN discovery. Suitable for
multi-operator dev/test setups where all peer addresses are known in advance.

`peer_addresses` lists the socket addresses of other operators in `<pubkey>@<host>:<port>` format.
`authorized_peers` lists the Ed25519 public keys of peers allowed to connect.

### Remote — Discovery Mode (production)

```toml
[wavs.p2p.remote]
listen_port = 9000
bootstrappers = []          # Empty = this node is a bootstrapper
authorized_peers = ["<hex_ed25519_pubkey>"]
max_message_size = 65536
deque_size = 128
```

Discovery mode uses bootstrapper nodes for initial peer discovery. Suitable for
multi-operator production deployments across the internet.

If `bootstrappers` is an empty list, this node acts as a bootstrapper for other peers.
Other nodes list its address in their `bootstrappers` field.

---

## Multi-Node Setup

### Local Development (2 operators on localhost)

Each operator needs their own `wavs.toml`. First, get each operator's Ed25519 public key
from their signing mnemonic:

```bash
wavs-cli p2p identity --mnemonic "your signing mnemonic words here"
# Output: <64-character hex Ed25519 public key>
```

**Node 1 (`wavs-node-1.toml`):**

```toml
[wavs]
port = 8041
signing_mnemonic = "<mnemonic_1>"

[wavs.p2p.local]
listen_port = 9000
peer_addresses = ["<node2_ed25519_pubkey>@127.0.0.1:9001"]
authorized_peers = ["<node2_ed25519_pubkey>"]
```

**Node 2 (`wavs-node-2.toml`):**

```toml
[wavs]
port = 8042
signing_mnemonic = "<mnemonic_2>"

[wavs.p2p.local]
listen_port = 9001
peer_addresses = ["<node1_ed25519_pubkey>@127.0.0.1:9000"]
authorized_peers = ["<node1_ed25519_pubkey>"]
```

Start both nodes. Use `GET /p2p/status` on each to confirm connectivity.

### Production Deployment

1. Designate one node as the initial bootstrapper (set `bootstrappers = []`).
2. All other nodes list the bootstrapper's address: `bootstrappers = ["<bootstrapper_pubkey>@<host>:9000"]`.
3. Exchange Ed25519 public keys out-of-band with all other operators.
4. Add all operators' public keys to `authorized_peers` in every node's config.

---

## Status Endpoint

`GET /p2p/status` returns the current P2P state of the node as JSON.

| Field | Type | Description |
|---|---|---|
| `enabled` | bool | Whether P2P networking is active |
| `local_peer_id` | string | Hex-encoded Ed25519 public key of this node |
| `listen_addresses` | string[] | Socket addresses this node listens on (e.g. `"0.0.0.0:9000"`) |
| `connected_peers` | number | Count of currently connected peers |
| `peer_ids` | string[] | Hex Ed25519 public keys of all connected peers |
| `subscribed_services` | string[] | Service names currently subscribed for P2P broadcast |

Example response:

```json
{
  "enabled": true,
  "local_peer_id": "a1b2c3d4e5f6...",
  "listen_addresses": ["0.0.0.0:9000"],
  "connected_peers": 2,
  "peer_ids": ["b2c3d4e5f6a1...", "c3d4e5f6a1b2..."],
  "subscribed_services": ["my-avs-service"]
}
```

---

## Identity

Ed25519 keypairs are derived deterministically from `WAVS_SIGNING_MNEMONIC` via ChaCha20Rng
seeded with the BIP-39 mnemonic seed bytes. The same mnemonic always produces the same
Ed25519 keypair and therefore the same peer ID.

**Peer ID format:** 64-character lowercase hex string (32-byte Ed25519 public key, hex-encoded).

**Address format:** `<peer_id>@<host>:<port>` — for example:
```
a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd@192.168.1.10:9000
```

To get your node's Ed25519 public key:
```bash
wavs-cli p2p identity --mnemonic "your signing mnemonic words here"
```
