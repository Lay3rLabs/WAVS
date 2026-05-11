# WAVS Agent Sidecar

Embedded AI assistant for the WAVS desktop app. Uses the [pi coding agent SDK](https://github.com/nicepkg/pi) to provide an LLM-powered developer command center.

## Architecture

The agent runs as a Node.js sidecar process spawned by the Tauri backend, communicating via JSON-RPC over stdio.

```
Tauri (Rust) ←── JSON-RPC/stdio ──→ Pi Sidecar (Node.js)
                                         │
                                         ├── wavs-tools extension (MCP client)
                                         │       └── spawns wavs-mcp binary (MCP/stdio)
                                         │
                                         └── ui-control extension
                                                 └── navigate, toast, clipboard, etc.
```

### Extensions

- **`wavs-tools.ts`** — Connects to `wavs-mcp` via MCP protocol over stdio. Provides all WAVS operations: build components, deploy services, query logs, manage operators, etc. The `wavs-mcp` binary is the single source of truth for WAVS operations.
- **`ui-control.ts`** — Tools for controlling the Tauri frontend: navigate pages, show toasts, copy to clipboard, open service detail views.

### Isolation

The sidecar is fully isolated from any user pi installation via `resourceLoaderOptions`:
- No user extensions, skills, prompt templates, or themes loaded
- Only the two bundled extensions above
- Sessions stored in `~/Library/Application Support/xyz.wavs/sessions/`
- Auth stored in `~/Library/Application Support/xyz.wavs/auth.json`

## Files

```
agent/
├── entrypoint.ts           # Main entry — creates session runtime + starts RPC mode
├── extensions/
│   ├── wavs-tools.ts       # MCP client for wavs-mcp
│   └── ui-control.ts       # UI control tools (navigate, toast, clipboard)
├── oauth-login.ts          # Standalone OAuth login script
├── package.json            # Dependencies (pi SDK, tsx)
└── tsconfig.json
```

## Development

Dependencies are installed automatically via the parent `app/package.json` postinstall script:

```bash
cd app && pnpm install   # Installs agent deps too
```

The sidecar is started/stopped by the Tauri backend. In dev mode (`#[cfg(debug_assertions)]`), it runs from the source `app/agent/` directory. In release builds, it uses the bundled copy in the app resources.

### Environment Variables (set by Tauri)

| Variable | Description |
|---|---|
| `WAVS_URL` | WAVS node API URL (e.g. `http://127.0.0.1:8041`) |
| `WAVS_HOME` | WAVS home directory (working directory for the agent) |
| `WAVS_MCP_BINARY` | Path to the `wavs-mcp` binary |
| `WAVS_MCP_TOKEN` | Bearer token for wavs-mcp (if configured) |
| `WAVS_AUTH_DIR` | Directory containing `auth.json` for LLM provider auth |

### RPC Protocol

The sidecar speaks pi's RPC protocol over stdin/stdout (newline-delimited JSON). Key commands:

- `prompt` — Send a user message
- `abort` — Cancel current generation
- `new_session` — Start a fresh session
- `switch_session` — Load a different session from disk
- `get_messages` — Retrieve current session messages
- `set_model` / `set_thinking` — Change model settings at runtime
