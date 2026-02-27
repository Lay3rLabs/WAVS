#!/usr/bin/env python3
"""
Register wavs-mcp with Claude Code for a given project directory.

Reads the running wavs-mcp process args (URL + token), finds the binary,
then upserts projects.<abs-path>.mcpServers.wavs in ~/.claude.json.
Chain credentials (mcp_chain_credential, signing_mnemonic) are written to
~/.wavs/wavs.toml — a user-level file outside any git repo, readable by
all MCP clients (Claude Code, Cursor, VS Code, etc.).

Usage:
    python3 ~/.claude/skills/wavs/setup_claude_mcp.py [/path/to/project]
    # default project = current working directory

    # From the WAVS repo:
    just setup-claude-mcp [/path/to/project]
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


# ---------------------------------------------------------------------------
# 1. Locate the wavs-mcp binary
# ---------------------------------------------------------------------------

def find_binary() -> str | None:
    """
    Find the wavs-mcp binary.

    Resolution order:
    1. target/{debug,release}/wavs-mcp relative to the WAVS repo root.
       Works when __file__ is .claude/skills/wavs/setup_claude_mcp.py
       inside a cloned WAVS repo (3 dirs up is the repo root).
    2. PATH via shutil.which (catches system installs and npm global installs).
    """
    # Compute candidate repo root: 3 levels up from __file__
    # (.claude/skills/wavs/setup_claude_mcp.py -> .claude/skills/wavs -> .claude/skills -> .claude -> repo root)
    script_path = Path(__file__).resolve()
    candidate_root = script_path.parent.parent.parent.parent
    for profile in ("debug", "release"):
        candidate = candidate_root / "target" / profile / "wavs-mcp"
        if candidate.exists():
            return str(candidate)

    # PATH fallback (covers npm global install and system install)
    found = shutil.which("wavs-mcp")
    if found:
        return found

    return None


# ---------------------------------------------------------------------------
# 2. Parse the running wavs-mcp process for --wavs-url and --token only
# ---------------------------------------------------------------------------

def parse_running_process() -> tuple[str | None, str | None]:
    """Return (wavs_url, token) from the running wavs-mcp process, or (None, None).
    Only reads --wavs-url and --token — non-sensitive connection parameters.
    """
    try:
        result = subprocess.run(
            ["ps", "aux"],
            capture_output=True, text=True, timeout=5
        )
        for line in result.stdout.splitlines():
            if "wavs-mcp" not in line or "grep" in line:
                continue
            url_m = re.search(r"--wavs-url\s+(\S+)", line)
            tok_m = re.search(r"--token\s+(\S+)", line)
            url = url_m.group(1) if url_m else None
            token = tok_m.group(1) if tok_m else None
            if url or token:
                return url, token
    except Exception:
        pass
    return None, None


# ---------------------------------------------------------------------------
# 3. Chain credentials: read from ~/.wavs/wavs.toml (user-home only)
# ---------------------------------------------------------------------------

def read_credentials_from_global_toml() -> tuple[str | None, str | None]:
    """
    Read mcp_chain_credential and signing_mnemonic from ~/.wavs/wavs.toml.
    Only reads from the user-home global credential store, never from
    project-local files (which may be inside a git repository).
    Returns (mcp_chain_credential, signing_mnemonic).
    """
    toml_path = Path.home() / ".wavs" / "wavs.toml"
    if not toml_path.exists():
        return None, None
    try:
        content = toml_path.read_text()

        def extract(key: str) -> str | None:
            m = re.search(rf'^{re.escape(key)}\s*=\s*"([^"]*)"', content, re.MULTILINE)
            if m:
                return m.group(1)
            m = re.search(rf"^{re.escape(key)}\s*=\s*'([^']*)'", content, re.MULTILINE)
            if m:
                return m.group(1)
            m = re.search(rf'^{re.escape(key)}\s*=\s*(\S+)', content, re.MULTILINE)
            if m:
                return m.group(1).strip("\"'")
            return None

        # Prefer new key name, fall back to legacy key for migration
        cred = extract("mcp_chain_credential") or extract("chain_write_credential")
        mnem = extract("signing_mnemonic")
        return cred, mnem
    except Exception:
        return None, None


# ---------------------------------------------------------------------------
# 4. Prompt for missing values
# ---------------------------------------------------------------------------

def prompt(label: str, default: str | None, secret: bool = False) -> str:
    if default:
        display = f"[{('*' * 8) if secret else default}]"
        raw = input(f"{label} {display}: ").strip()
        return raw if raw else default
    else:
        while True:
            raw = input(f"{label}: ").strip()
            if raw:
                return raw
            print("  (required — please enter a value)")


def prompt_optional(label: str, default: str | None, secret: bool = False) -> str | None:
    """Like prompt() but allows empty input to mean 'skip'."""
    if default:
        display = f"[{('*' * 8) if secret else default}]"
        raw = input(f"{label} {display} (Enter to keep): ").strip()
        return raw if raw else default
    else:
        raw = input(f"{label} (optional — press Enter to skip): ").strip()
        return raw if raw else None


# ---------------------------------------------------------------------------
# 5. Write credentials to ~/.wavs/wavs.toml
# ---------------------------------------------------------------------------

def _upsert_toml_key(content: str, section: str, key: str, value: str) -> str:
    """Insert or update `key = "value"` in [section] of TOML content.

    If the section doesn't exist, appends it. If the key doesn't exist in
    the section, appends it. If it exists, replaces the value in-place.
    """
    new_assignment = f'{key} = "{value}"'
    lines = content.splitlines(keepends=True)

    in_target_section = False
    section_found = False
    key_found = False
    insert_before_idx = None  # Insert key before this line if not found in section

    for i, line in enumerate(lines):
        stripped = line.strip()
        # Detect section headers (not array-of-tables [[...]])
        if stripped.startswith("[") and not stripped.startswith("[["):
            current_section = stripped[1:stripped.index("]")].strip()
            if current_section == section:
                in_target_section = True
                section_found = True
            elif in_target_section:
                # We've left the target section
                in_target_section = False
                insert_before_idx = i
                break

        if in_target_section and not stripped.startswith("["):
            if re.match(rf"^{re.escape(key)}\s*=", stripped):
                lines[i] = new_assignment + "\n"
                key_found = True
                break

    if key_found:
        return "".join(lines)

    if section_found:
        # Section exists but key was not found
        if insert_before_idx is not None:
            # Insert before the next section header
            lines.insert(insert_before_idx, new_assignment + "\n")
        else:
            # Section extends to EOF; append to end of file
            sep = "" if content.endswith("\n") else "\n"
            return "".join(lines) + sep + new_assignment + "\n"
        return "".join(lines)
    else:
        # Section doesn't exist at all; append it
        sep = "" if not content or content.endswith("\n") else "\n"
        return "".join(lines) + sep + f"[{section}]\n{new_assignment}\n"


def write_credentials_to_wavs_toml(
    cred: str | None,
    mnem: str | None,
) -> None:
    """Write mcp_chain_credential and signing_mnemonic to ~/.wavs/wavs.toml."""
    toml_path = Path.home() / ".wavs" / "wavs.toml"
    toml_path.parent.mkdir(parents=True, exist_ok=True)
    content = toml_path.read_text() if toml_path.exists() else ""
    if cred:
        content = _upsert_toml_key(content, "wavs", "mcp_chain_credential", cred)
    if mnem:
        content = _upsert_toml_key(content, "wavs", "signing_mnemonic", mnem)
    toml_path.write_text(content)


# ---------------------------------------------------------------------------
# 6. Atomically update ~/.claude.json
# ---------------------------------------------------------------------------

def update_claude_json(
    project_path: str,
    command: str,
    args: list[str],
) -> None:
    claude_json = Path.home() / ".claude.json"

    if claude_json.exists():
        with claude_json.open() as f:
            try:
                config = json.load(f)
            except json.JSONDecodeError:
                config = {}
    else:
        config = {}

    config.setdefault("projects", {})
    config["projects"].setdefault(project_path, {})
    config["projects"][project_path].setdefault("mcpServers", {})

    entry: dict = {
        "command": command,
        "args": args,
    }

    config["projects"][project_path]["mcpServers"]["wavs"] = entry

    tmp = tempfile.NamedTemporaryFile(
        mode="w",
        dir=claude_json.parent,
        delete=False,
        suffix=".tmp",
    )
    try:
        json.dump(config, tmp, indent=2)
        tmp.write("\n")
        tmp.close()
        os.replace(tmp.name, claude_json)
    except Exception:
        os.unlink(tmp.name)
        raise


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    # Determine project path
    if len(sys.argv) >= 2:
        project_path = str(Path(sys.argv[1]).resolve())
    else:
        project_path = str(Path.cwd().resolve())

    print(f"Project path : {project_path}")

    # Locate binary
    binary = find_binary()
    if not binary:
        print("\nCould not find wavs-mcp binary.")
        print("Build it:    cargo build -p wavs-mcp")
        print("Or install:  npm install -g @lay3rlabs/wavs-mcp")
        sys.exit(1)
    print(f"Binary       : {binary}")

    # Get URL + token from running process (non-sensitive connection params only)
    url, token = parse_running_process()
    if url:
        print(f"Detected URL : {url}")
    if token:
        print("Detected token from running process.")

    # Prompt for anything still missing
    if not url:
        url = prompt("wavs-mcp URL", "http://localhost:8000")
    if not token:
        token = prompt("Token (--token arg)", None, secret=True)

    args = ["--wavs-url", url, "--token", token]

    # --- Credentials: read from ~/.wavs/wavs.toml, prompt if missing ---
    print("\nLooking for chain credentials in ~/.wavs/wavs.toml ...")

    cred, mnem = read_credentials_from_global_toml()
    if cred or mnem:
        print("  Credentials found.")
    else:
        print("  Not found — enter them now or press Enter to skip.")
        cred = prompt_optional("  mcp_chain_credential (private key for gas)", None, secret=True)
        mnem = prompt_optional("  signing_mnemonic (node signing key)", None, secret=True)

    # Update ~/.claude.json (command + args only, no credentials)
    update_claude_json(project_path, binary, args)

    print("\nDone! Written to ~/.claude.json:")
    print(json.dumps({
        "projects": {
            project_path: {
                "mcpServers": {
                    "wavs": {"command": binary, "args": args}
                }
            }
        }
    }, indent=2))

    # Write credentials to ~/.wavs/wavs.toml (universal — all MCP clients read this)
    if cred or mnem:
        write_credentials_to_wavs_toml(cred, mnem)
        print("\nCredentials written to ~/.wavs/wavs.toml")
        print("wavs-mcp reads them automatically from all MCP clients (Claude Code, Cursor, VS Code, etc.).")

    print("\nRestart Claude Code (or reload MCP servers) to pick up the change.")


if __name__ == "__main__":
    main()
