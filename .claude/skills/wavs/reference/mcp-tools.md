# MCP Tools Reference

All tools are exposed by the `wavs` MCP server. Prefix with `wavs:` when calling (e.g. `wavs:wavs_get_node_info`).

---

## Auth Requirements

| Tool | Token (`--token`) | Chain Cred | Dev Endpoints | Notes |
|------|:-----------------:|:----------:|:-------------:|-------|
| `wavs_get_node_info` | — | — | — | Service count, chain keys, aggregator config, P2P status |
| `wavs_get_health` | — | — | — | Health of all configured chain RPC endpoints |
| `wavs_list_services` | — | — | — | All registered services with workflows, triggers, components |
| `wavs_get_service` | — | — | — | Requires `chain` (e.g. `"evm:31337"`) + `address` params |
| `wavs_deploy_service` | ✓ | — | — | Reads service def from chain via ServiceManager |
| `wavs_delete_service` | ✓ | — | — | Permanently removes service from WAVS node |
| `wavs_pause_service` | ✓ | — | — | Halts trigger execution; service stays registered |
| `wavs_resume_service` | ✓ | — | — | Re-enables a paused service |
| `wavs_set_service_uri` | — | ✓ | — | EVM only; calls `setServiceURI` on ServiceManager contract |
| `wavs_deploy_service_manager` | — | ✓ | — | EVM only; deploys `SimpleServiceManager.sol`; returns `address` |
| `wavs_deploy_poa_service_manager` | — | ✓ | — | EVM only; deploys `POAStakeRegistry` proxy via Docker; returns proxy `address` |
| `wavs_register_operator` | — | ✓ + mnemonic | — | EVM only; registers node's signing key on POAStakeRegistry |
| `wavs_upload_component` | — | — | ✓ | Uploads `.wasm` binary; returns raw 64-char hex digest (no `sha256:` prefix) |
| `wavs_save_service` | — | — | ✓ | Saves service def to node store; returns URI |
| `wavs_simulate_trigger` | — | — | ✓ | Fires a test trigger against a deployed service |
| `wavs_deploy_dev_service` | — | — | ✓ | Registers service directly without on-chain contract |
| `wavs_query_kv` | — | — | ✓ | Reads a value from a service's KV store |
| `wavs_get_wit_interface` | — | — | — | Returns full WIT interface definitions (local, no network) |
| `wavs_scaffold_component` | — | — | — | Generates Cargo.toml + src/lib.rs skeleton (local) |
| `wavs_build_component` | — | — | — | Runs `cargo component build`; returns build output (local) |

**Legend:**
- Token: MCP server must be started with `--token <value>`; pass token in requests
- Chain Cred: `WAVS_MCP_CHAIN_CREDENTIAL` env var must be set in the MCP client's `"env"` block (or `~/.wavs/wavs.toml` as fallback)
- Dev Endpoints: `dev_endpoints_enabled = true` must be set under `[wavs]` in `wavs.toml`

---

## Tool Parameter Details

### wavs_get_service
```
chain:   "evm:31337" or "cosmos:mychain"
address: "0xServiceManagerAddress..."
```

### wavs_deploy_service / wavs_delete_service / wavs_pause_service / wavs_resume_service
```
service_manager_json: see reference/service-json.md
```

### wavs_set_service_uri
```
service_manager_json: {"evm": {"chain": "evm:31337", "address": "0x..."}}
uri:                  URI returned by wavs_save_service
rpc_url:              RPC endpoint for the chain (e.g. "http://localhost:8545")
```

### wavs_deploy_service_manager / wavs_deploy_poa_service_manager
```
rpc_url: RPC endpoint for the chain (e.g. "http://localhost:8545")
```
Returns: contract address (use as `address` in service_manager_json)

### wavs_register_operator
```
service_manager_json: {"evm": {"chain": "evm:31337", "address": "0x..."}}
weight:               optional uint64 (default: 100)
rpc_url:              RPC endpoint for the chain (e.g. "http://localhost:8545")
```

### wavs_upload_component
```
file_path: absolute path to .wasm file
```
Returns: `"Component uploaded.\nDigest: <64-char hex>"` — the digest is a raw hex string with **no** `sha256:` prefix. Use it directly in `component.source.digest`.

### wavs_save_service / wavs_deploy_dev_service
```
service_json: full service definition JSON string
```
See [`service-json.md`](service-json.md) for the full schema.

### wavs_simulate_trigger
```
service_id:   64-char hex string (from wavs_list_services)
workflow_id:  lowercase alphanumeric, 3–36 chars (e.g. "default")
trigger_json: trigger definition (see service-json.md)
data_json:    trigger data payload (see service-json.md)
count:        optional, how many times to fire (default: 1)
```

### wavs_query_kv
```
service_id: 64-char hex string
bucket:     KV bucket name (as passed to store::open in component)
key:        key within the bucket
```

### wavs_build_component
```
dir:     directory containing the component's Cargo.toml
release: optional bool (default: true)
```

### wavs_scaffold_component
```
name:         lowercase-with-hyphens component name
trigger_type: evm_contract_event | cosmos_contract_event | block_interval | cron | manual
description:  optional string
```

---

## MCP Server Configuration

The MCP server binary is `wavs-mcp`. Key CLI args:

| Arg | Description |
|-----|-------------|
| `--wavs-url <url>` | WAVS node HTTP API URL (e.g. `http://localhost:8000`) |
| `--token <token>` | Auth token (enables write tools) |

The WAVS node URL and token can also be found by inspecting the running `wavs-mcp` process:
```bash
ps aux | grep wavs-mcp
```

Environment variables:
- `WAVS_URL` — WAVS node URL
- `WAVS_TOKEN` — auth token
- `WAVS_MCP_CHAIN_CREDENTIAL` — credential for on-chain ops (falls back to `mcp_chain_credential` in `~/.wavs/wavs.toml`)
- `WAVS_SIGNING_MNEMONIC` — signing mnemonic (falls back to `signing_mnemonic` in `~/.wavs/wavs.toml`)
