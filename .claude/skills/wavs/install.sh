#!/usr/bin/env bash
set -euo pipefail
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
git clone --depth=1 --filter=blob:none --no-checkout \
  https://github.com/Lay3rLabs/wavs.git "$TMP/wavs" 2>/dev/null
cd "$TMP/wavs"
git sparse-checkout init --cone
git sparse-checkout set .claude/skills/wavs
git checkout
mkdir -p ~/.claude/skills
cp -r .claude/skills/wavs ~/.claude/skills/wavs
echo "WAVS skill installed to ~/.claude/skills/wavs"
echo "Restart Claude Code to pick up the skill."
echo ""
echo "Next: register wavs-mcp with Claude Code."
echo "Run from any project directory:"
echo "  npx @wavs/mcp@latest"
echo ""
echo "This interactive wizard installs wavs-mcp, writes ~/.claude.json,"
echo "and writes ~/.wavs/wavs.toml so chain-write tools work from any project."
echo ""
echo "WAVS repo users: 'just setup-claude-mcp [/path/to/project]' does the same."
