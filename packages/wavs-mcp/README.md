# wavs-mcp

[Model Context Protocol](https://modelcontextprotocol.io) server for the WAVS platform.

Exposes WAVS operations to AI assistants (Claude, Cursor, VS Code Copilot, etc.) — scaffold and build WASM components, upload binaries, deploy services, simulate triggers, and query a live WAVS node, all from natural language.

---

## Installation

### One-command setup (recommended)

```bash
npx @wavs/mcp@latest
```

Interactive wizard: installs the binary, prompts for URL + token + credentials, writes `~/.claude.json` and `~/.wavs/wavs.toml`, and installs Claude Code skill files.

### Global install

```bash
npm install -g @wavs/mcp
```

The postinstall script downloads the correct pre-built binary for your platform.

### Build from source

```bash
cargo build --release -p wavs-mcp
# Binary: ./target/release/wavs-mcp
```

---

## Running

```bash
wavs-mcp --wavs-url http://localhost:8000 --token <your-token>
```

### All flags

| Flag | Env var | Default | Description |
|------|---------|---------|-------------|
| `--wavs-url` | `WAVS_URL` | `http://localhost:8000` | WAVS node HTTP API URL |
| `--token` | `WAVS_TOKEN` | — | Bearer token for write operations |
| `--mcp-chain-credential` | `WAVS_MCP_CHAIN_CREDENTIAL` | — | Private key (`0x…`) or BIP39 mnemonic for on-chain transactions |
| `--signing-mnemonic` | `WAVS_SIGNING_MNEMONIC` | — | BIP39 mnemonic for the WAVS node signing key |

All flags can be set as environment variables — useful for running standalone without exposing secrets in `ps aux`.

---

## Credential Storage

On-chain tools (`wavs_deploy_service_manager`, `wavs_register_operator`, `wavs_set_service_uri`, etc.) need `mcp_chain_credential` and/or `signing_mnemonic`. Recommended storage in priority order:

### 1. `~/.wavs/wavs.toml` (recommended — works with all MCP clients)

```toml
[wavs]
mcp_chain_credential = "0x<private-key>"
signing_mnemonic = "word1 word2 ... word12"
```

`wavs-mcp` reads this file automatically. Only `~/.wavs/wavs.toml` is searched — project-local `wavs.toml` files are intentionally skipped to prevent accidental credential commits.

The WAVS desktop app's "Register with Claude" button and `just setup-claude-mcp` write this file automatically.

### 2. Environment variables (per-client overrides)

Set in your shell or in the MCP client's `"env"` block:

```bash
export WAVS_MCP_CHAIN_CREDENTIAL="0x<private-key>"
export WAVS_SIGNING_MNEMONIC="word1 word2 ... word12"
```

### 3. CLI flags (avoid — visible in `ps aux`)

```bash
wavs-mcp --mcp-chain-credential 0x... --signing-mnemonic "word1 ..."
```

---

## Client Configuration

### Claude Code

```bash
# One-command setup (recommended)
npx @wavs/mcp@latest

# From the WAVS repo
just setup-claude-mcp /path/to/your-project
```

### Claude Desktop (macOS)

Edit `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "wavs": {
      "command": "wavs-mcp",
      "args": ["--wavs-url", "http://localhost:8000", "--token", "your-token"]
    }
  }
}
```

Credentials are read automatically from `~/.wavs/wavs.toml`.

### Claude Desktop (Linux)

Same config, edit `~/.config/Claude/claude_desktop_config.json`.

### Cursor

Edit `.cursor/mcp.json` (project) or `~/.cursor/mcp.json` (global):

```json
{
  "mcpServers": {
    "wavs": {
      "command": "wavs-mcp",
      "args": ["--wavs-url", "http://localhost:8000", "--token", "your-token"]
    }
  }
}
```

### VS Code (Copilot / MCP extension)

Edit `.vscode/mcp.json`:

```json
{
  "servers": {
    "wavs": {
      "type": "stdio",
      "command": "wavs-mcp",
      "args": ["--wavs-url", "http://localhost:8000", "--token", "your-token"]
    }
  }
}
```

---

## Tools

| Category | Tools | Auth |
|----------|-------|------|
| **Read** | `wavs_get_node_info`, `wavs_get_health`, `wavs_list_services`, `wavs_get_service` | None |
| **Write** | `wavs_deploy_service`, `wavs_delete_service` | `--token` |
| **Dev** | `wavs_upload_component`, `wavs_save_service`, `wavs_simulate_trigger`, `wavs_deploy_dev_service`, `wavs_query_kv` | Dev endpoints enabled |
| **Chain-write** | `wavs_deploy_service_manager`, `wavs_deploy_poa_service_manager`, `wavs_register_operator`, `wavs_set_service_uri` | `mcp_chain_credential` |
| **Local** | `wavs_get_wit_interface`, `wavs_scaffold_component`, `wavs_build_component`, `wavs_get_service_schema` | None |

Local tools run entirely on the client machine — no running WAVS node needed.

---

## Full Documentation

See [`MCP.md`](../../MCP.md) in the repo root for complete tool reference, parameter shapes, and end-to-end workflow examples.
