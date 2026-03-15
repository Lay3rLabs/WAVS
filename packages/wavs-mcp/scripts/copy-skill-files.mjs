#!/usr/bin/env node
/**
 * scripts/copy-skill-files.mjs — prepack helper.
 * Copies .claude/skills/wavs/ → packages/wavs-mcp/skill/ before npm publish.
 * Safe to run in an unpacked tarball context (exits silently if source missing).
 */

import { cpSync, existsSync, mkdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// scripts/ → wavs-mcp/ → packages/ → repo root
const repoRoot = path.resolve(__dirname, '../../..');
const src = path.join(repoRoot, '.claude', 'skills', 'wavs');
const dest = path.join(__dirname, '..', 'skill');

if (!existsSync(src)) {
  // Not in a full repo checkout (e.g. running from an unpacked tarball) — skip silently.
  process.exit(0);
}

mkdirSync(dest, { recursive: true });
cpSync(src, dest, { recursive: true, force: true });
console.log(`Copied ${src} → ${dest}`);
