#!/usr/bin/env node
/**
 * postinstall.mjs — downloads the platform-specific wavs-mcp binary from
 * the GitHub release that matches the package version.
 *
 * Runs once automatically after `npm install @lay3rlabs/wavs-mcp`.
 */

import { createWriteStream, chmodSync, existsSync, mkdirSync } from 'fs';
import { pipeline } from 'stream/promises';
import { createGunzip } from 'zlib';
import { Extract } from 'tar';
import path from 'path';
import { fileURLToPath } from 'url';
import { readFileSync } from 'fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Read version from our own package.json
const pkg = JSON.parse(readFileSync(path.join(__dirname, 'package.json'), 'utf8'));
const VERSION = pkg.version;

// Map Node.js platform/arch to the release asset name produced by the CI matrix.
function getAssetName() {
  const platform = process.platform;
  const arch = process.arch;

  const targets = {
    'darwin-arm64':  'wavs-mcp-aarch64-apple-darwin.tar.gz',
    'darwin-x64':    'wavs-mcp-x86_64-apple-darwin.tar.gz',
    'linux-x64':     'wavs-mcp-x86_64-unknown-linux-gnu.tar.gz',
    'linux-arm64':   'wavs-mcp-aarch64-unknown-linux-gnu.tar.gz',
    'win32-x64':     'wavs-mcp-x86_64-pc-windows-msvc.zip',
  };

  const key = `${platform}-${arch}`;
  const asset = targets[key];
  if (!asset) {
    throw new Error(
      `Unsupported platform/arch: ${key}. ` +
      `Build wavs-mcp from source: https://github.com/Lay3rLabs/wavs`
    );
  }
  return asset;
}

async function fetchWithRedirects(url, maxRedirects = 5) {
  const { default: https } = await import('https');
  const { default: http } = await import('http');

  return new Promise((resolve, reject) => {
    let redirects = 0;

    function doRequest(currentUrl) {
      const mod = currentUrl.startsWith('https') ? https : http;
      mod.get(currentUrl, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          if (++redirects > maxRedirects) {
            return reject(new Error('Too many redirects'));
          }
          return doRequest(res.headers.location);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} downloading ${currentUrl}`));
        }
        resolve(res);
      }).on('error', reject);
    }

    doRequest(url);
  });
}

async function downloadBinary() {
  const asset = getAssetName();
  const url = `https://github.com/Lay3rLabs/wavs/releases/download/v${VERSION}/${asset}`;
  const binDir = path.join(__dirname, 'bin');
  const isWindows = process.platform === 'win32';
  const binaryName = isWindows ? 'wavs-mcp.exe' : 'wavs-mcp';
  const binaryPath = path.join(binDir, binaryName);

  if (existsSync(binaryPath)) {
    console.log(`wavs-mcp binary already present at ${binaryPath}`);
    return;
  }

  mkdirSync(binDir, { recursive: true });

  console.log(`Downloading wavs-mcp v${VERSION} for ${process.platform}/${process.arch}...`);
  console.log(`  URL: ${url}`);

  const response = await fetchWithRedirects(url);

  if (asset.endsWith('.tar.gz')) {
    // Stream through gunzip + tar extract, grab only the binary
    await pipeline(
      response,
      createGunzip(),
      new Extract({
        cwd: binDir,
        filter: (p) => path.basename(p) === 'wavs-mcp' || path.basename(p) === 'wavs-mcp.exe',
      })
    );
  } else {
    // .zip on Windows — write zip then extract
    const { default: https } = await import('https');
    const zipPath = path.join(binDir, 'wavs-mcp.zip');
    await pipeline(response, createWriteStream(zipPath));

    // Node 18.3+ has a built-in unzip via child_process on Windows; use it
    const { execFileSync } = await import('child_process');
    execFileSync('powershell', [
      '-Command',
      `Expand-Archive -Path "${zipPath}" -DestinationPath "${binDir}" -Force`,
    ]);
    import('fs').then(({ unlinkSync }) => {
      try { unlinkSync(zipPath); } catch {}
    });
  }

  if (!isWindows) {
    chmodSync(binaryPath, 0o755);
  }

  console.log(`wavs-mcp installed to ${binaryPath}`);
}

downloadBinary().catch((err) => {
  console.error(`Failed to download wavs-mcp binary: ${err.message}`);
  console.error('You can build it from source: cargo build --release -p wavs-mcp');
  // Non-fatal — the package still installs; bin/run.mjs will error at runtime.
  process.exit(0);
});
