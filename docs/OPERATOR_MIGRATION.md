# Operator Migration Guide: libp2p to commonware

This guide covers migrating a WAVS operator node from the libp2p P2P backend
to the new commonware backend.

## Prerequisites

- Current WAVS node running with libp2p P2P (or planning an upgrade from a version that used it)
- Coordination with all operators in your deployment
- Access to your `WAVS_SIGNING_MNEMONIC` (same mnemonic used for on-chain signing)

## Breaking Changes

### 1. Identity Format Change

- **Old:** secp256k1 keypair, peer IDs like `12D3KooW...` (base58-encoded multihash)
- **New:** Ed25519 keypair derived from `WAVS_SIGNING_MNEMONIC` via ChaCha20Rng (BIP-39 seed)
- Peer IDs are now hex-encoded Ed25519 public keys (64 lowercase hex characters)
- The same mnemonic produces a **different** peer ID from before (different key algorithm and derivation)
- To view your new peer ID:

```bash
wavs-cli p2p identity --mnemonic "your signing mnemonic words here"
# Output: a1b2c3d4e5f6... (64 hex characters)
```

### 2. Address Format Change

- **Old:** multiaddr format — `/ip4/1.2.3.4/tcp/9000/p2p/12D3KooW...`
- **New:** socket format — `<hex_ed25519_pubkey>@<host>:<port>`
- Example: `a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd@192.168.1.10:9000`

### 3. Configuration Format Change

**Old wavs.toml (libp2p):**

```toml
# Local mode (mDNS):
[wavs.p2p]
local = { listen_port = 9000 }

# Remote mode (Kademlia DHT):
[wavs.p2p]
remote = { listen_port = 9000, bootstrap_nodes = ["/ip4/1.2.3.4/tcp/9000/p2p/12D3Koo..."] }
```

**New wavs.toml (commonware):**

```toml
# Local mode (lookup, known peer addresses):
[wavs.p2p.local]
listen_port = 9000
peer_addresses = ["<hex_ed25519_pubkey>@127.0.0.1:9001"]
authorized_peers = ["<hex_ed25519_pubkey>"]
max_message_size = 65536    # optional, default 64 KB
deque_size = 128            # optional, default 128 messages

# Remote mode (discovery, bootstrapper-based):
[wavs.p2p.remote]
listen_port = 9000
bootstrappers = ["<hex_ed25519_pubkey>@1.2.3.4:9000"]
authorized_peers = ["<hex_ed25519_pubkey>"]
max_message_size = 65536    # optional
deque_size = 128            # optional
```

New fields added:
- `authorized_peers`: Required. List of Ed25519 public keys allowed to connect. Replaces the implicit "known peers" concept from libp2p.
- `peer_addresses` (Local mode only): Explicit peer addresses replacing mDNS automatic discovery.
- `bootstrappers` (Remote mode only): Replaces `bootstrap_nodes` with the new address format.
- `max_message_size`: Optional. Maximum P2P message size in bytes (default: 65536).
- `deque_size`: Optional. Broadcast message cache size per peer for catch-up on reconnection (default: 128).

### 4. Discovery Mechanism Change

- **Old:** mDNS for automatic LAN peer discovery (local mode), Kademlia DHT for production
- **New:** Lookup mode requires explicit `peer_addresses` (no automatic LAN discovery), Discovery mode uses bootstrapper-based peer resolution
- There is no automatic peer discovery in the new system. Peers must be explicitly listed (Local mode) or reachable via bootstrappers (Remote mode).

## Coordinated Upgrade Requirement

**All operators in a deployment MUST upgrade simultaneously.**

The old (libp2p) and new (commonware) backends use incompatible wire protocols, identity
schemes, and address formats. A libp2p node and a commonware node cannot communicate.

Plan a maintenance window with all operators. The window requires:
1. Stopping all nodes
2. Upgrading all binaries
3. Updating all configurations with new Ed25519 peer IDs
4. Starting all nodes together

Rolling upgrades are not possible for the P2P networking layer.

## Step-by-Step Migration

### 1. Coordinate with Other Operators

Schedule a maintenance window with all operators in your deployment. Agree on:
- Upgrade time (all nodes must be stopped and upgraded together)
- How to exchange new Ed25519 public keys (email, shared doc, secure channel)
- Rollback plan if issues arise

### 2. Generate Your New Ed25519 Identity

Before the maintenance window, generate your new peer ID:

```bash
wavs-cli p2p identity --mnemonic "your signing mnemonic words here"
```

Share the output (64-character hex string) with all other operators in your deployment.
Collect their hex public keys in return. You will need these for `authorized_peers`
and `peer_addresses`/`bootstrappers` in the new config.

### 3. Stop All Operator Nodes

During the maintenance window, stop all WAVS operator nodes in your deployment.
Verify all nodes are stopped before proceeding.

### 4. Update WAVS Binary

Download and install the new WAVS release that includes the commonware P2P backend.
Verify the version:

```bash
wavs-node --version
```

### 5. Update wavs.toml Configuration

Edit your `wavs.toml` to use the new P2P configuration format.

**For Local (development/testing) deployments:**

```toml
[wavs.p2p.local]
listen_port = 9000
peer_addresses = ["<other_operator_pubkey>@<host>:<port>"]
authorized_peers = ["<other_operator_pubkey>"]
```

**For Remote (production) deployments — bootstrapper node:**

```toml
[wavs.p2p.remote]
listen_port = 9000
bootstrappers = []              # Empty: this node is the bootstrapper
authorized_peers = [
    "<operator2_pubkey>",
    "<operator3_pubkey>",
]
```

**For Remote (production) deployments — other nodes:**

```toml
[wavs.p2p.remote]
listen_port = 9000
bootstrappers = ["<bootstrapper_pubkey>@<bootstrapper_host>:9000"]
authorized_peers = [
    "<operator1_pubkey>",       # bootstrapper
    "<operator3_pubkey>",
]
```

Replace `<*_pubkey>` placeholders with the actual 64-character hex Ed25519 public keys
collected from each operator in step 2.

### 6. Update authorized_peers on All Nodes

Every operator must add all other operators' Ed25519 public keys to their
`authorized_peers` list. Only peers listed in `authorized_peers` can connect.
A node that is not in another node's `authorized_peers` will be rejected.

### 7. Start All Operator Nodes

Start all nodes simultaneously (or within a few seconds of each other). For Remote
mode, start the bootstrapper node first, wait a moment, then start the other nodes.

### 8. Verify Connectivity

Use the P2P status endpoint to confirm all nodes are connected:

```bash
curl http://localhost:8041/p2p/status | jq .
```

Expected response:

```json
{
  "enabled": true,
  "local_peer_id": "<your_64_char_hex_pubkey>",
  "listen_addresses": ["0.0.0.0:9000"],
  "connected_peers": 2,
  "peer_ids": ["<operator2_pubkey>", "<operator3_pubkey>"],
  "subscribed_services": ["my-avs-service"]
}
```

- `connected_peers` should match the number of other operators (N - 1 for an N-operator deployment)
- `peer_ids` should list all other operators' Ed25519 public keys
- `subscribed_services` should show all registered AVS services

## Verification Checklist

- [ ] `GET /p2p/status` returns `"enabled": true` on all nodes
- [ ] `connected_peers` equals the expected number (total operators minus 1)
- [ ] `peer_ids` lists all other operators' hex public keys
- [ ] `subscribed_services` shows all registered services
- [ ] Trigger processing resumes: check logs for `Aggregator: quorum reached` messages
- [ ] On-chain submissions resume: verify transactions are being posted

## Rollback

If the upgrade fails, rolling back requires ALL operators to revert simultaneously
to the old binary. The old libp2p and new commonware backends are incompatible —
a partial rollback leaves the deployment non-functional.

To rollback:
1. Stop all nodes
2. Restore old WAVS binary on all nodes
3. Restore old `wavs.toml` on all nodes (keep the old libp2p config)
4. Start all nodes simultaneously

## Reference

- [P2P.md](P2P.md) — Full P2P configuration reference for the commonware backend
- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture overview
- `wavs.toml` — Config template with all P2P options documented
