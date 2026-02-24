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

## Claude Code Integration

WAVS ships with a `/wavs` slash command for [Claude Code](https://claude.ai/code) that gives Claude a full understanding of the WAVS component development workflow, including how to scaffold, build, upload, and deploy components using the MCP tools.

### In-repo usage (automatic)

If you're working inside this repository, the `/wavs` command is available automatically — Claude Code picks up `.claude/commands/wavs.md` from the project root. No installation needed.

### Global installation

To use `/wavs` in any Claude Code session (outside this repo):

```bash
cp .claude/commands/wavs.md ~/.claude/commands/wavs.md
```

### MCP server

The `/wavs` command works best paired with the `wavs-mcp` server, which exposes live WAVS node operations (scaffold, build, upload, deploy, simulate) as MCP tools. See [MCP.md](MCP.md) for setup instructions.

---
For more guides, architecture details, and examples, see the [docs folder](docs/README.md).
