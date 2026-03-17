# WAVS

![Banner](docs/images/wavs.png)

[![Project Status: Active -- The project has reached a stable, usable state and is being actively developed.](https://img.shields.io/badge/repo%20status-Active-green.svg?style=flat-square)](https://www.repostatus.org/#active)

## What is WAVS?

WAVS is a next-generation platform for building and running **Actively Validated Services (AVS)**. It provides a robust infrastructure so you can focus on your service logic, not boilerplate.

**Key Features:**
- Write services in Rust (more languages coming soon)
- Compile to WebAssembly (WASM) for portability and speed
- Deploy as lightweight service components
- Trigger execution from blockchains, clocks, or other events
- Run offchain at near-native speed in the WAVS WASI runtime
- Bring results verifiably onchain
- Dynamically manage multiple components for flexible, intelligent applications

**What is AVS?**
An Actively Validated Service (AVS) is a decentralized service that is validated by independent operators. WAVS makes it easy to create, manage, and operate high-performance AVSs.

**Why WASM?**
WebAssembly (WASM) enables fast, secure, and portable execution of code across platforms. WAVS leverages WASM to run your services efficiently and safely.

---

## Release Workflow

This project uses [just](https://github.com/casey/just) for managing releases. Here's the typical workflow:

1. **Make your changes** — develop features, fix bugs, update WIT definitions, etc.
2. **Set the version** — update version numbers across `Cargo.toml` and all WIT package definitions:
   ```bash
   just set-version v2.7.0
   ```
3. **Create PR and merge** — get your changes reviewed and merged to main
4. **Push tags** — after merge, create and push git tags:
   ```bash
   just push-tag v2.7.0
   ```

This creates both a standard tag (`v2.7.0`) and a Go module tag (`wasi/go/v2.7.0`). The standard tag triggers CI to publish the `wavs-types` and `wavs-wasi-utils` crates to crates.io, WASM components to wa.dev, and TypeScript bindings to NPM.

---

## Claude Code Integration

WAVS ships with a `/wavs` skill for [Claude Code](https://claude.ai/code) that gives Claude a full understanding of the WAVS component development workflow, including how to scaffold, build, upload, and deploy components using the MCP tools.

Full Claude Code integration requires two independent steps:

1. **Install the skill** — teaches Claude the WAVS workflow and tool reference.
2. **Register `wavs-mcp`** — connects Claude Code to a live WAVS node so MCP tools actually work.

### Step 1: Install the skill

**In-repo (automatic):** If you're working inside this repository, the `/wavs` skill is available automatically. No installation needed.

**Global (repo cloned):**
```bash
just install-claude-skill
```

**Global (no clone needed):**
```bash
bash <(curl -fsSL https://raw.githubusercontent.com/Lay3rLabs/wavs/main/.claude/skills/wavs/install.sh)
```

After installation, restart Claude Code to pick up the skill.

### Step 2: Register wavs-mcp with Claude Code

The skill's MCP tools require `wavs-mcp` to be running and registered for each project. Run once per project directory:

```bash
# From the WAVS repo — auto-detects the running wavs-mcp process:
just setup-claude-mcp /path/to/your-project
```

This writes the `mcpServers.wavs` entry into `~/.claude.json` for that project. Restart Claude Code (or reload MCP servers) afterwards.

See [MCP.md](MCP.md) for full setup and configuration details.

---
For more guides, architecture details, and examples, see the [docs folder](docs/README.md).
