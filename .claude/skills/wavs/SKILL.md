---
name: wavs
description: >
  Develops, deploys, and manages WAVS (WebAssembly-based Actively Validated Services)
  components and services using the wavs MCP server. Use when the user wants to build a
  WASM component, deploy a new service, update an existing service, or manage service
  lifecycle. Triggers on: wavs, wasm component, AVS, service manager,
  deploy service, scaffold component.
---

# WAVS Developer Guide

WAVS runs off-chain computation as sandboxed WebAssembly (WASI) components triggered by on-chain events (EVM/Cosmos contract events, cron, block intervals, or manual inputs); execution results are submitted back on-chain through a ServiceManager contract.

---

## Deterministic On-Chain Reads

Treat the trigger/event block height as the canonical snapshot for on-chain data that relates to that trigger. When a component performs EVM, Cosmos, or other RPC reads and the client/API supports it, pass the trigger block number or block height explicitly instead of reading `latest` state.

Avoid mixing historical trigger data with unpinned latest-state reads. Pinning reads to the trigger block improves replayability and prevents nondeterministic state/timestamp mismatches, such as an oracle response whose `updatedAt` appears newer than the block timestamp because an RPC backend served cached state from a different snapshot.

---

## MCP Setup

The `wavs:` MCP tools below require `wavs-mcp` to be running and registered with Claude Code.

**One-command setup (recommended)**
```bash
npx @wavs/mcp@latest
```
Interactive wizard: installs the binary, prompts for URL + token + credentials,
writes `~/.claude.json` and `~/.wavs/wavs.toml`, installs skill files.

**Using the WAVS desktop app**
The app auto-starts `wavs-mcp`. Use the "Register with Claude Code" button in
Settings → MCP Server to register for any project path without leaving the app.

**WAVS repo users**
```bash
just setup-claude-mcp [/path/to/project]
```

**CLI / manual setup**
```bash
# 1. Start a WAVS node
just start-wavs-dev

# 2. Run wavs-mcp (in a separate terminal)
./target/release/wavs-mcp --wavs-url http://localhost:8000 --token <token>

# 3. Register with Claude Code
npx @wavs/mcp@latest
```

> **Local tools** (`scaffold_component`, `build_component`, `get_wit_interface`) work without MCP — useful for component development without a running node.

---

## Choose Your Flow

| User Intent | Follow |
|-------------|--------|
| Build a new component from scratch | [`flows/component-dev.md`](flows/component-dev.md) |
| Deploy a new service with an on-chain contract | [`flows/deployment.md`](flows/deployment.md) |
| Update a deployed service with a new component | [`flows/update-service.md`](flows/update-service.md) |

When in doubt, start with **component-dev** — it ends with a deployment step.

---

## MCP Tool Categories

| Category | Tools | Auth Required |
|----------|-------|---------------|
| **Read** | `get_node_info`, `get_health`, `list_services`, `get_service` | None |
| **Write** | `deploy_service`, `delete_service` | `--token` |
| **Dev** | `upload_component`, `save_service`, `simulate_trigger`, `deploy_dev_service`, `query_kv` | Dev endpoints enabled |
| **Chain-write** | `set_service_uri`, `deploy_service_manager`, `deploy_poa_service_manager`, `register_operator` | `WAVS_MCP_CHAIN_CREDENTIAL` env var |
| **Local** | `get_wit_interface`, `scaffold_component`, `build_component` | None |

Full tool reference: [`reference/mcp-tools.md`](reference/mcp-tools.md)

---

## Key Configuration

For on-chain operations, credentials are read from `~/.wavs/wavs.toml` (the WAVS home config):

```toml
[wavs]
mcp_chain_credential = "0x<private-key>"
signing_mnemonic = "word1 word2 ... word12"
```

The WAVS app "Register with Claude" button and `just setup-claude-mcp` write this file automatically.

Dev endpoints must be enabled in `wavs.toml` under `[wavs]`:
```toml
dev_endpoints_enabled = true   # Required for upload, save, simulate, deploy_dev
```

---

## Reference

- [`reference/mcp-tools.md`](reference/mcp-tools.md) — All 20 tools with auth requirements and parameter notes
- [`reference/service-json.md`](reference/service-json.md) — Service/trigger JSON formats + simulate examples
