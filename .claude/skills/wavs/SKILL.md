---
name: wavs
description: >
  Develops, deploys, and manages WAVS (WebAssembly-based Actively Validated Services)
  components and services using the wavs MCP server. Use when the user wants to build a
  WASM component, deploy a new service, update an existing service, or manage service
  lifecycle (pause/resume). Triggers on: wavs, wasm component, AVS, service manager,
  deploy service, scaffold component.
---

# WAVS Developer Guide

WAVS runs off-chain computation as sandboxed WebAssembly (WASI) components triggered by on-chain events (EVM/Cosmos contract events, cron, block intervals, or manual inputs); execution results are submitted back on-chain through a ServiceManager contract.

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
| **Write** | `deploy_service`, `delete_service`, `pause_service`, `resume_service` | `--token` |
| **Dev** | `upload_component`, `save_service`, `simulate_trigger`, `deploy_dev_service`, `query_kv` | Dev endpoints enabled |
| **Chain-write** | `set_service_uri`, `deploy_service_manager`, `deploy_poa_service_manager`, `register_operator` | `--token` + `chain_write_credential` |
| **Local** | `get_wit_interface`, `scaffold_component`, `build_component` | None |

Full tool reference: [`reference/mcp-tools.md`](reference/mcp-tools.md)

---

## Key Configuration

`wavs.toml` must have these set for on-chain operations:

```toml
chain_write_credential = "0x<private-key>"   # Pays gas; deploys contracts; owns PoA registry
signing_mnemonic = "word1 word2 ... word12"  # Node's signing key (HD index 0 used by default)
dev_endpoints_enabled = true                 # Required for upload, save, simulate, deploy_dev
```

---

## Reference

- [`reference/mcp-tools.md`](reference/mcp-tools.md) — All 20 tools with auth requirements and parameter notes
- [`reference/service-json.md`](reference/service-json.md) — Service/trigger JSON formats + simulate examples
