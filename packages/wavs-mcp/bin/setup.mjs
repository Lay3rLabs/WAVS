#!/usr/bin/env node
/**
 * bin/setup.mjs — interactive wizard for `npx @wavs/mcp@latest` (no-args).
 * Uses only Node.js built-ins: readline/promises, child_process, fs, path, os.
 */

import { createInterface } from 'readline/promises';
import { execFileSync, execSync, spawnSync } from 'child_process';
import { existsSync, mkdirSync, cpSync, readFileSync, writeFileSync, renameSync } from 'fs';
import path from 'path';
import os from 'os';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function banner() {
  console.log('');
  console.log('  WAVS MCP Setup Wizard');
  console.log('  ─────────────────────');
  console.log('  Sets up wavs-mcp for Claude Code');
  console.log('');
}

/** Find a locally built wavs-mcp binary in the repo's target/ directory. */
function findLocalBinary() {
  const binaryName = process.platform === 'win32' ? 'wavs-mcp.exe' : 'wavs-mcp';
  // packages/wavs-mcp/bin/ → packages/wavs-mcp/ → packages/ → repo root
  const repoRoot = path.resolve(__dirname, '../../..');
  for (const profile of ['release', 'debug']) {
    const candidate = path.join(repoRoot, 'target', profile, binaryName);
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

/** Find wavs-mcp binary in a permanent (non-npx-cache) location: PATH or npm global. */
function findGlobalBinary() {
  const binaryName = process.platform === 'win32' ? 'wavs-mcp.exe' : 'wavs-mcp';

  try {
    const result = spawnSync(process.platform === 'win32' ? 'where' : 'which', ['wavs-mcp'], {
      encoding: 'utf8',
    });
    if (result.status === 0 && result.stdout.trim()) return result.stdout.trim().split('\n')[0].trim();
  } catch {}

  try {
    const npmRoot = execFileSync('npm', ['root', '-g'], { encoding: 'utf8' }).trim();
    const candidate = path.join(npmRoot, '.bin', binaryName);
    if (existsSync(candidate)) return candidate;
  } catch {}

  return null;
}

/** Parse --wavs-url and --token from any running wavs-mcp process. */
function detectRunningProcess() {
  try {
    const out = execSync('ps aux', { encoding: 'utf8', timeout: 5000 });
    for (const line of out.split('\n')) {
      if (!line.includes('wavs-mcp') || line.includes('grep')) continue;
      const urlM = line.match(/--wavs-url\s+(\S+)/);
      const tokM = line.match(/--token\s+(\S+)/);
      if (urlM || tokM) {
        return { url: urlM?.[1] ?? null, token: tokM?.[1] ?? null };
      }
    }
  } catch {}
  return { url: null, token: null };
}

/** Upsert key = "value" in [section] of TOML text. */
function upsertTomlKey(content, section, key, value) {
  const newAssignment = `${key} = "${value}"`;
  const lines = content.split('\n');
  let inTarget = false;
  let sectionFound = false;
  let keyFound = false;
  let insertBeforeIdx = null;

  for (let i = 0; i < lines.length; i++) {
    const stripped = lines[i].trim();
    if (stripped.startsWith('[') && !stripped.startsWith('[[')) {
      const name = stripped.slice(1, stripped.indexOf(']')).trim();
      if (name === section) {
        inTarget = true;
        sectionFound = true;
      } else if (inTarget) {
        inTarget = false;
        insertBeforeIdx = i;
        break;
      }
    }
    if (inTarget && !stripped.startsWith('[')) {
      if (new RegExp(`^${key.replace('.', '\\.')}\\s*=`).test(stripped)) {
        lines[i] = newAssignment;
        keyFound = true;
        break;
      }
    }
  }

  if (keyFound) return lines.join('\n');

  if (sectionFound) {
    if (insertBeforeIdx !== null) {
      lines.splice(insertBeforeIdx, 0, newAssignment);
    } else {
      const sep = content.endsWith('\n') ? '' : '\n';
      return lines.join('\n') + sep + newAssignment + '\n';
    }
    return lines.join('\n');
  }

  // Section doesn't exist — append it
  const sep = !content || content.endsWith('\n') ? '' : '\n';
  return lines.join('\n') + sep + `[${section}]\n${newAssignment}\n`;
}

/** Read mcp_chain_credential and signing_mnemonic from ~/.wavs/wavs.toml. */
function readWavsToml() {
  const tomlPath = path.join(os.homedir(), '.wavs', 'wavs.toml');
  if (!existsSync(tomlPath)) return { cred: null, mnem: null };
  try {
    const content = readFileSync(tomlPath, 'utf8');
    const extract = (k) => {
      const m = content.match(new RegExp(`^${k}\\s*=\\s*"([^"]*)"`, 'm'))
        || content.match(new RegExp(`^${k}\\s*=\\s*'([^']*)'`, 'm'))
        || content.match(new RegExp(`^${k}\\s*=\\s*(\\S+)`, 'm'));
      return m ? m[1].replace(/^["']|["']$/g, '') : null;
    };
    const cred = extract('mcp_chain_credential') || extract('chain_write_credential');
    const mnem = extract('signing_mnemonic');
    return { cred, mnem };
  } catch {
    return { cred: null, mnem: null };
  }
}

/** Write credentials to ~/.wavs/wavs.toml. */
function writeWavsToml(cred, mnem) {
  const tomlPath = path.join(os.homedir(), '.wavs', 'wavs.toml');
  mkdirSync(path.dirname(tomlPath), { recursive: true });
  let content = existsSync(tomlPath) ? readFileSync(tomlPath, 'utf8') : '';
  if (cred) content = upsertTomlKey(content, 'wavs', 'mcp_chain_credential', cred);
  if (mnem) content = upsertTomlKey(content, 'wavs', 'signing_mnemonic', mnem);
  writeFileSync(tomlPath, content);
}

/** Atomically write ~/.claude.json with the MCP server entry. */
function writeClaudeJson(projectPath, command, args, global_) {
  const claudeJson = path.join(os.homedir(), '.claude.json');
  let config = {};
  if (existsSync(claudeJson)) {
    try { config = JSON.parse(readFileSync(claudeJson, 'utf8')); } catch {}
  }

  const entry = { command, args };

  if (global_) {
    config.mcpServers = config.mcpServers || {};
    config.mcpServers.wavs = entry;
  } else {
    config.projects = config.projects || {};
    config.projects[projectPath] = config.projects[projectPath] || {};
    config.projects[projectPath].mcpServers = config.projects[projectPath].mcpServers || {};
    config.projects[projectPath].mcpServers.wavs = entry;
  }

  const tmp = claudeJson + '.tmp';
  writeFileSync(tmp, JSON.stringify(config, null, 2) + '\n');
  renameSync(tmp, claudeJson);
}

/** Copy bundled skill/ directory to ~/.claude/skills/wavs/. */
function installSkillFiles() {
  const skillSrc = path.join(__dirname, '..', 'skill');
  if (!existsSync(skillSrc)) {
    console.log('  (skill files not bundled in this install — skipping)');
    return false;
  }
  const dest = path.join(os.homedir(), '.claude', 'skills', 'wavs');
  mkdirSync(path.dirname(dest), { recursive: true });
  cpSync(skillSrc, dest, { recursive: true, force: true });
  return true;
}

// ---------------------------------------------------------------------------
// Main wizard
// ---------------------------------------------------------------------------

export default async function main() {
  banner();

  const rl = createInterface({ input: process.stdin, output: process.stdout });

  const ask = async (question, defaultVal, hidden = false) => {
    const hint = defaultVal
      ? ` [${hidden ? '********' : defaultVal}]`
      : '';
    const answer = await rl.question(`  ${question}${hint}: `);
    return answer.trim() || defaultVal || '';
  };

  try {
    // 1. Binary — prefer local repo build, then global install
    const localBinary = findLocalBinary();
    let binary;
    if (localBinary) {
      console.log(`  Local build found: ${localBinary}`);
      binary = localBinary;
    } else {
      binary = findGlobalBinary();
      if (!binary) {
        console.log('  wavs-mcp not found in PATH.');
      }
      const installAns = await rl.question(
        binary
          ? `  wavs-mcp found at ${binary}. Reinstall/update globally? [y/N] `
          : '  Install wavs-mcp globally via npm install -g @wavs/mcp? [Y/n] '
      );
      const shouldInstall = binary
        ? installAns.trim().toLowerCase() === 'y'
        : !installAns.trim() || installAns.trim().toLowerCase() === 'y';
      if (shouldInstall) {
        console.log('  Running: npm install -g @wavs/mcp ...');
        execFileSync('npm', ['install', '-g', '@wavs/mcp'], { stdio: 'inherit' });
        binary = findGlobalBinary();
      }
      if (!binary) {
        console.error('\n  Could not find wavs-mcp binary. Aborting.');
        process.exit(1);
      }
    }
    console.log(`  Binary: ${binary}`);
    console.log('');

    // 2. Scope
    console.log('  Scope:');
    console.log('    1) Global (all Claude Code sessions)');
    console.log('    2) Current project only');
    const scopeAns = await rl.question('  Choose [1]: ');
    const global_ = (scopeAns.trim() || '1') !== '2';
    const projectPath = process.cwd();
    console.log(`  Scope: ${global_ ? 'global' : `project (${projectPath})`}`);
    console.log('');

    // 3. Detect running process
    const { url: detectedUrl, token: detectedToken } = detectRunningProcess();
    if (detectedUrl) console.log(`  Detected running wavs-mcp at ${detectedUrl}`);

    // 4. URL
    const url = await ask('wavs-mcp URL', detectedUrl || 'http://localhost:8000');

    // 5. Token
    let token = detectedToken;
    if (!token) {
      token = await ask('Token (--token)', null, true);
    } else {
      console.log('  Token: detected from running process');
    }
    console.log('');

    // 6. Credentials
    console.log('  Checking ~/.wavs/wavs.toml for chain credentials ...');
    const { cred: existingCred, mnem: existingMnem } = readWavsToml();
    let cred = existingCred;
    let mnem = existingMnem;

    if (existingCred || existingMnem) {
      console.log('  Credentials found.');
      const override = await rl.question('  Override them? [y/N] ');
      if (override.trim().toLowerCase() === 'y') {
        const newCred = await ask('mcp_chain_credential (private key, optional)', null, true);
        const newMnem = await ask('signing_mnemonic (BIP39 mnemonic, optional)', null, true);
        if (newCred) cred = newCred;
        if (newMnem) mnem = newMnem;
      }
    } else {
      console.log('  Not found — enter them now (or press Enter to skip).');
      const newCred = await ask('mcp_chain_credential (private key, optional)', null, true);
      const newMnem = await ask('signing_mnemonic (BIP39 mnemonic, optional)', null, true);
      if (newCred) cred = newCred;
      if (newMnem) mnem = newMnem;
    }
    console.log('');

    // 7. Write ~/.wavs/wavs.toml
    if (cred || mnem) {
      writeWavsToml(cred || null, mnem || null);
      console.log('  Credentials written to ~/.wavs/wavs.toml');
    }

    // 8. Install skill files
    const skInstalled = installSkillFiles();
    if (skInstalled) console.log('  Skill files installed to ~/.claude/skills/wavs/');

    // 9. Write ~/.claude.json
    const args = ['--wavs-url', url, '--token', token];
    writeClaudeJson(projectPath, binary, args, global_);
    console.log('  MCP server entry written to ~/.claude.json');

    // 10. Summary
    console.log('');
    console.log('  Done!');
    console.log('  Restart Claude Code to pick up changes.');
    console.log('');
  } finally {
    rl.close();
  }
}

// Self-invocation guard
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((err) => {
    console.error(err.message);
    process.exit(1);
  });
}
