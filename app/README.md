# WAVS Desktop App

Tauri 2 desktop application for managing WAVS nodes, services, and operators. Includes an embedded AI agent for developer assistance.

## Architecture

- **Frontend**: React 19 + Vite 7 + Zustand + Tailwind CSS
- **Backend**: Tauri 2 (Rust) — manages the WAVS node, keychain, and sidecar processes
- **Agent**: Embedded [pi coding agent](https://github.com/nicepkg/pi) sidecar (TypeScript) communicating via JSON-RPC over stdio
- **wavs-mcp**: MCP server providing WAVS tools to the agent (build components, deploy services, query logs, etc.)

## Development

### Prerequisites

- Rust toolchain (via `rustup`)
- Node.js 20+
- pnpm (`corepack enable && corepack prepare pnpm@latest --activate`)

### Quick Start

```bash
# From repo root:
cd app && pnpm install   # Installs frontend + agent deps (agent via postinstall)
just wavs-mcp-build      # Build the MCP server (needed by the agent)
just app-dev             # Launches Tauri dev with hot reload
```

### Other Commands

```bash
just app-dev-frontend    # Vite frontend dev server only (no Tauri)
just app-build-release   # Production build
just app-build-frontend  # Vite build only
```

## Agent Setup

The embedded agent lives in `agent/` and is automatically installed via the `postinstall` script. It requires:

1. **LLM API key** — configured in the app's Settings page (supports Anthropic OAuth or API keys for various providers)
2. **wavs-mcp binary** — build with `just wavs-mcp-build`; the app locates it in `target/{debug,release}/wavs-mcp`

See [`agent/README.md`](agent/README.md) for agent architecture details.

## Project Structure

```
app/
├── src/                    # React frontend
│   ├── components/
│   │   ├── agent/          # Agent panel UI (chat, tool calls, input)
│   │   ├── atoms/          # Shared UI primitives (Button, Toast, etc.)
│   │   └── layout/         # Header, Body, resize handle
│   ├── pages/              # Route pages (Services, Logs, Health, Settings)
│   ├── stores/             # Zustand stores (app, agent, wallet, etc.)
│   ├── tauri/              # Tauri command wrappers + event listeners
│   └── hooks/              # React hooks
├── src-tauri/              # Rust backend
│   └── src/
│       ├── lib.rs          # Tauri setup + state management
│       ├── agent.rs        # Pi sidecar process management + RPC relay
│       ├── commands.rs     # All Tauri commands
│       ├── logger.rs       # Tracing → frontend log forwarding
│       └── state.rs        # Shared state types
├── agent/                  # Pi coding agent sidecar (TypeScript)
└── public/                 # Static assets
```
