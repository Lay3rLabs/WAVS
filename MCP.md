# WAVS MCP Server

`wavs-mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server that exposes WAVS platform operations to AI clients over stdio. It lets Claude Desktop, Cursor, VS Code, and other MCP-compatible clients query a live WAVS node, scaffold and build WASM components, upload binaries, deploy services, and simulate triggers — all from natural language.

---

## Quick Start

### 1. Build the binary

```bash
cargo build --release -p wavs-mcp
# Binary: ./target/release/wavs-mcp
```

### 2. Start a WAVS node (if you don't have one running)

```bash
just start-wavs-dev
```

### 3. Add to your client config (see below)

### 4. Test with MCP Inspector

```bash
npx @modelcontextprotocol/inspector ./target/release/wavs-mcp
```

---

## Client Configuration

### Claude Desktop (macOS)

Edit `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "wavs": {
      "command": "/path/to/WAVS/target/release/wavs-mcp",
      "args": [],
      "env": {
        "WAVS_URL": "http://localhost:8000",
        "WAVS_TOKEN": "your-token-here"
      }
    }
  }
}
```

### Claude Desktop (Linux)

Edit `~/.config/Claude/claude_desktop_config.json` with the same structure above.

### Cursor

Edit `.cursor/mcp.json` in your project root (or `~/.cursor/mcp.json` globally):

```json
{
  "mcpServers": {
    "wavs": {
      "command": "/path/to/WAVS/target/release/wavs-mcp",
      "args": ["--wavs-url", "http://localhost:8000"],
      "env": {
        "WAVS_TOKEN": "your-token-here"
      }
    }
  }
}
```

### VS Code (Copilot / MCP extension)

Edit `.vscode/mcp.json` in your workspace:

```json
{
  "servers": {
    "wavs": {
      "type": "stdio",
      "command": "/path/to/WAVS/target/release/wavs-mcp",
      "args": [],
      "env": {
        "WAVS_URL": "http://localhost:8000",
        "WAVS_TOKEN": "your-token-here"
      }
    }
  }
}
```

### CLI flags (alternative to env vars)

```bash
./target/release/wavs-mcp \
  --wavs-url http://localhost:8000 \
  --token your-bearer-token
```

---

## Tool Reference

Tools are grouped into five categories.

### Read tools — no auth required

| Tool | Description | Parameters |
|------|-------------|------------|
| `wavs_get_node_info` | Node info: service count, chain keys, aggregator config, P2P status | none |
| `wavs_get_health` | Health status of all configured chain RPC endpoints | none |
| `wavs_list_services` | All registered services with workflows, triggers, and components | none |
| `wavs_get_service` | Full config for one service | `chain` (e.g. `"evm:31337"`), `address` (contract address) |

### Write tools — require `--token`

| Tool | Description | Parameters |
|------|-------------|------------|
| `wavs_deploy_service` | Register a service from its ServiceManager | `service_manager_json` (see format below) |
| `wavs_pause_service` | Pause a running service | `service_manager_json` |
| `wavs_resume_service` | Resume a paused service | `service_manager_json` |

`service_manager_json` shapes:
```json
// EVM
{"evm": {"chain": "evm:31337", "address": "0xAbCd..."}}

// Cosmos
{"cosmos": {"chain": "cosmos:mychain", "address": "cosmos1..."}}
```

### Chain-write tools — require `--token` + `chain_write_credential` on the node

> **Note:** These tools send real on-chain transactions. They require both a valid `--token` *and* `chain_write_credential` to be set in `wavs.toml` (or via `WAVS_CHAIN_WRITE_CREDENTIAL`). The endpoint returns **403** if the credential is not configured. EVM only currently.

| Tool | Description | Parameters |
|------|-------------|------------|
| `wavs_set_service_uri` | Call `setServiceURI` on the ServiceManager contract to update the on-chain service URI | `service_manager_json`, `uri` |
| `wavs_deploy_service_manager` | Deploy a new `SimpleServiceManager` PoA contract on-chain; returns `address` and `tx_hash` | `chain` (e.g. `"evm:31337"`) |
| `wavs_deploy_poa_service_manager` | Deploy a full `POAStakeRegistry` (upgradeable proxy) via Docker; returns proxy `address` (use as service manager). Requires Docker with `ghcr.io/lay3rlabs/poa-middleware:1.0.1`. | `chain` |
| `wavs_register_operator` | Register the WAVS node's signing key as an operator on a `POAStakeRegistry` and set the signing key mapping. Calls `registerOperator` (owner) + `updateOperatorSigningKey` (operator). Requires `chain_write_credential` + `signing_mnemonic` configured. | `service_manager_json`, `weight` (optional, default 100) |

To enable, add to `wavs.toml` under `[wavs]`:
```toml
# Credential (private key or mnemonic) for on-chain management transactions.
# Env var override: WAVS_CHAIN_WRITE_CREDENTIAL
chain_write_credential = "0x<private_key_or_mnemonic>"

# Signing mnemonic for WAVS operator key derivation (used by wavs_register_operator)
# Env var override: WAVS_SIGNING_MNEMONIC
signing_mnemonic = "<mnemonic>"
```

### Dev tools — require dev endpoints enabled in `wavs.toml`

> **Note:** Dev tools require `dev_endpoints_enabled = true` under `[wavs]` in `wavs.toml`. Restart the WAVS node after changing this.

| Tool | Description | Parameters |
|------|-------------|------------|
| `wavs_upload_component` | Upload a compiled `.wasm` binary; returns its digest | `file_path` (absolute path to `.wasm`) |
| `wavs_simulate_trigger` | Fire a trigger against a deployed service | `service_id`, `workflow_id`, `trigger_json`, `data_json`, `count` (optional) |
| `wavs_deploy_dev_service` | Register a service directly without an on-chain contract | `service_json` (full Service definition as JSON string) |
| `wavs_query_kv` | Read a value from a service's KV store | `service_id`, `bucket`, `key` |

`wavs_simulate_trigger` parameter shapes:

```json
// trigger_json examples
{"manual": null}
{"cron": {"schedule": "* * * * *", "start_time": null, "end_time": null}}
{"evm_contract_event": {"chain": "evm:31337", "address": "0x...", "event_hash": "0x<32-byte-keccak>"}}

// data_json examples
{"Raw": [104, 101, 108, 108, 111]}
{"Cron": {"trigger_time": 0}}
{"EvmContractEvent": {"log": {...}}}
```

### Local tools — run entirely on the client machine

| Tool | Description | Parameters |
|------|-------------|------------|
| `wavs_get_wit_interface` | Returns the full WIT interface definitions (HTTP, KV, TLS, host functions, etc.) | none |
| `wavs_scaffold_component` | Generate a ready-to-build component scaffold (Cargo.toml + lib.rs) | `name`, `trigger_type`, `description` (optional) |
| `wavs_build_component` | Build a component with `cargo component build`; returns full stdout/stderr | `dir` (path to component), `release` (default: true) |

`wavs_scaffold_component` trigger types: `evm_contract_event` | `cosmos_contract_event` | `block_interval` | `cron` | `manual`

---

## Example Prompts

Paste any of these directly into Claude Desktop (or any MCP-connected client) after configuring the server.

**Inspect your node:**
> "What services are currently running on my WAVS node?"

> "Show me the health of my chain RPC endpoints."

> "Get the WIT interface so I know what APIs are available in WASM components."

**Scaffold and build:**
> "Scaffold a WASM component called `price-feed` that processes EVM contract events."

> "Build the component at `examples/components/price-feed`."

**Upload and deploy:**
> "Upload the wasm at `./target/wasm32-wasip2/release/price_feed.wasm`."

> "Deploy the service at address `0xABCD1234...` on chain `evm:31337`."

**Simulate:**
> "Simulate a raw trigger with data bytes `[104, 101, 108, 108, 111]` against service `abc123...` workflow `default`."

**Lifecycle:**
> "Pause the service at `0xABCD...` on `evm:31337`."

> "Resume the paused service at `0xABCD...` on `evm:31337`."

---

## End-to-End Workflow

This sequence takes you from a blank slate to a running on-chain service.

```
1.  Start node          just start-wavs-dev
2.  Get WIT interface   wavs_get_wit_interface
                        → understand available APIs before writing code
3.  Scaffold            wavs_scaffold_component {name, trigger_type}
                        → writes Cargo.toml + src/lib.rs skeleton
4.  Implement           Edit src/lib.rs — decode trigger, compute, encode output
5.  Build               wavs_build_component {dir}
                        → cargo component build --release; read errors, fix, repeat
6.  Upload              wavs_upload_component {file_path}
                        → returns component digest (sha256:...)
7.  Deploy              wavs_deploy_service {service_manager_json}
                        → registers the service; WAVS reads config from chain
8.  Simulate            wavs_simulate_trigger {service_id, workflow_id, trigger_json, data_json}
                        → fires a test trigger; check logs for output
9.  Verify              wavs_list_services / wavs_get_node_info
                        → confirm the service is running
```

---

## System Prompt for Agentic Mode

Paste this as a system prompt to put an AI assistant into WAVS developer mode:

```
You are a WAVS (WebAssembly-based Actively Validated Services) developer assistant.
You have access to the following MCP tools via the wavs server:

Read (no auth): wavs_get_node_info, wavs_get_health, wavs_list_services, wavs_get_service
Write (require token): wavs_deploy_service, wavs_pause_service, wavs_resume_service, wavs_delete_service
Chain-write (require token + chain_write_credential): wavs_set_service_uri, wavs_deploy_service_manager
Dev (require dev endpoints): wavs_upload_component, wavs_simulate_trigger, wavs_deploy_dev_service, wavs_query_kv
Local: wavs_get_wit_interface, wavs_scaffold_component, wavs_build_component

When the user asks you to create a component, follow this workflow:
1. Call wavs_get_wit_interface to understand available APIs.
2. Call wavs_scaffold_component to generate the project skeleton.
3. Help the user implement the logic in src/lib.rs.
4. Call wavs_build_component — read the output, fix any errors, rebuild.
5. Call wavs_upload_component with the compiled .wasm path.
6. Call wavs_deploy_service with the ServiceManager JSON.
7. Call wavs_simulate_trigger to test the service.
8. Call wavs_list_services to confirm it's running.

Components are Rust libraries compiled to wasm32-wasip2. They implement the `Guest` trait,
decode trigger data with `decode_trigger_event`, process it, and return encoded responses
with `encode_trigger_output`. State is stored via wasi::keyvalue; outbound HTTP uses wasi::http.
The authoritative WIT interface definitions live in the wavs-wasi repo; use `wavs_get_wit_interface`
to retrieve the local copy bundled with this node.
```
