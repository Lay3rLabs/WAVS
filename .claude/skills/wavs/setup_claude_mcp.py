#!/usr/bin/env python3
"""
Register wavs-mcp with Claude Code for a given project directory.

Reads the running wavs-mcp process args (URL + token), finds the binary,
then upserts projects.<abs-path>.mcpServers.wavs in ~/.claude.json.
Also writes chain credentials to ~/.wavs/wavs.toml so chain-write tools
work from any project without a local wavs.toml.

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
# 2. Parse the running wavs-mcp process for --wavs-url and --token
# ---------------------------------------------------------------------------

def parse_running_process() -> tuple[str | None, str | None]:
    """Return (wavs_url, token) from the running wavs-mcp process, or (None, None)."""
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
# 3. Chain credentials: read from running process or wavs.toml
# ---------------------------------------------------------------------------

def read_credentials_from_running() -> tuple[str | None, str | None]:
    """
    Parse --chain-write-credential and --signing-mnemonic from ps aux output.
    Returns (chain_write_credential, signing_mnemonic).
    """
    try:
        result = subprocess.run(
            ["ps", "aux"],
            capture_output=True, text=True, timeout=5
        )
        for line in result.stdout.splitlines():
            if "wavs-mcp" not in line or "grep" in line:
                continue
            cred_m = re.search(r"--chain-write-credential\s+(\S+)", line)
            # signing-mnemonic may be a multi-word phrase in quotes; grab to next flag or end
            mnem_m = re.search(r'--signing-mnemonic\s+"([^"]+)"', line)
            if not mnem_m:
                mnem_m = re.search(r"--signing-mnemonic\s+'([^']+)'", line)
            if not mnem_m:
                mnem_m = re.search(r"--signing-mnemonic\s+(\S+)", line)
            cred = cred_m.group(1) if cred_m else None
            mnem = mnem_m.group(1) if mnem_m else None
            if cred or mnem:
                return cred, mnem
    except Exception:
        pass
    return None, None


def read_credentials_from_toml(path: Path) -> tuple[str | None, str | None]:
    """
    Read chain_write_credential and signing_mnemonic from [wavs] section
    of a wavs.toml file. Returns (chain_write_credential, signing_mnemonic).
    """
    if not path.exists():
        return None, None
    try:
        content = path.read_text()

        def extract(key: str) -> str | None:
            # Match key = "value" or key = 'value' or key = value
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

        return extract("chain_write_credential"), extract("signing_mnemonic")
    except Exception:
        return None, None


def write_wavs_toml(cred: str | None, mnem: str | None) -> None:
    """
    Upsert chain_write_credential and signing_mnemonic in ~/.wavs/wavs.toml.
    Creates the file and directory if needed. No-ops if both values are None.
    """
    if not cred and not mnem:
        return

    wavs_dir = Path.home() / ".wavs"
    wavs_dir.mkdir(exist_ok=True)
    toml_path = wavs_dir / "wavs.toml"

    content = toml_path.read_text() if toml_path.exists() else ""

    def upsert_key(text: str, key: str, value: str) -> str:
        # If key already exists anywhere, update it in place
        pattern = rf'^({re.escape(key)}\s*=\s*).*$'
        new_text, n = re.subn(pattern, f'{key} = "{value}"', text, flags=re.MULTILINE)
        if n > 0:
            return new_text
        # Append inside existing [wavs] section, or create section
        if "[wavs]" in text:
            idx = text.index("[wavs]") + len("[wavs]")
            # Find the start of the next section header
            next_section = re.search(r'^\[(?!wavs\b)', text[idx:], re.MULTILINE)
            if next_section:
                insert_at = idx + next_section.start()
                return text[:insert_at] + f'{key} = "{value}"\n' + text[insert_at:]
            else:
                return text.rstrip() + f'\n{key} = "{value}"\n'
        else:
            return text.rstrip() + f'\n\n[wavs]\n{key} = "{value}"\n'

    if cred:
        content = upsert_key(content, "chain_write_credential", cred)
    if mnem:
        content = upsert_key(content, "signing_mnemonic", mnem)

    # Write atomically
    tmp = tempfile.NamedTemporaryFile(
        mode="w",
        dir=wavs_dir,
        delete=False,
        suffix=".tmp",
    )
    try:
        tmp.write(content)
        tmp.close()
        os.replace(tmp.name, toml_path)
    except Exception:
        os.unlink(tmp.name)
        raise

    print(f"Credentials written to {toml_path}")


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
# 5. Atomically update ~/.claude.json
# ---------------------------------------------------------------------------

def update_claude_json(project_path: str, command: str, args: list[str]) -> None:
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

    config["projects"][project_path]["mcpServers"]["wavs"] = {
        "command": command,
        "args": args,
    }

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

    # Get URL + token from running process
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

    # Update ~/.claude.json
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

    # --- Credentials for ~/.wavs/wavs.toml ---
    print("\nLooking for chain credentials for ~/.wavs/wavs.toml ...")

    # 1. Try running process args
    cred, mnem = read_credentials_from_running()

    # 2. Try ./wavs.toml in CWD
    if not cred and not mnem:
        local_toml = Path.cwd() / "wavs.toml"
        cred, mnem = read_credentials_from_toml(local_toml)
        if (cred or mnem) and local_toml.exists():
            print(f"  Credentials found in {local_toml}")

    # 3. Interactive prompt
    if not cred and not mnem:
        print("  Not found automatically — enter them now or press Enter to skip.")
        cred = prompt_optional("  chain_write_credential (private key for gas)", None, secret=True)
        mnem = prompt_optional("  signing_mnemonic (node signing key)", None, secret=True)

    write_wavs_toml(cred, mnem)

    print("\nRestart Claude Code (or reload MCP servers) to pick up the change.")


if __name__ == "__main__":
    main()
