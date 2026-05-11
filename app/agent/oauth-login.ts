/**
 * OAuth login script — spawned by Tauri to run an OAuth flow for a provider.
 *
 * Usage: npx tsx oauth-login.ts <provider-id> <auth-json-path>
 *
 * Outputs JSON lines on stdout:
 *   {"type":"open_url","url":"https://..."}     — open this URL in the user's browser
 *   {"type":"progress","message":"..."}         — status update
 *   {"type":"success","provider":"..."}         — login complete, credentials saved
 *   {"type":"error","message":"..."}            — login failed
 */

import { AuthStorage } from "@mariozechner/pi-coding-agent";
import { exec } from "node:child_process";

const providerId = process.argv[2];
const authJsonPath = process.argv[3];

function output(obj: Record<string, unknown>) {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

function openUrl(url: string) {
  const cmd = process.platform === "darwin"
    ? `open "${url}"`
    : process.platform === "win32"
      ? `start "" "${url}"`
      : `xdg-open "${url}"`;
  console.error(`[oauth-login] Opening browser: ${cmd}`);
  exec(cmd, (err) => {
    if (err) console.error(`[oauth-login] Failed to open browser: ${err.message}`);
    else console.error(`[oauth-login] Browser opened successfully`);
  });
}

if (!providerId || !authJsonPath) {
  output({ type: "error", message: "Usage: oauth-login.ts <provider-id> <auth-json-path>" });
  process.exit(1);
}

const authStorage = AuthStorage.create(authJsonPath);
const providers = authStorage.getOAuthProviders();
const provider = providers.find((p) => p.id === providerId);

if (!provider) {
  const available = providers.map((p) => `${p.id} (${p.name})`);
  output({
    type: "error",
    message: `No OAuth provider "${providerId}". Available: ${available.join(", ")}`,
  });
  process.exit(1);
}

output({ type: "progress", message: `Starting ${provider.name} login...` });

try {
  await authStorage.login(providerId, {
    onAuth: (info) => {
      output({ type: "open_url", url: info.url, instructions: info.instructions });
      // Actually open the browser
      openUrl(info.url);
    },
    onPrompt: async (prompt) => {
      output({ type: "prompt", message: prompt.message, placeholder: prompt.placeholder });
      // Read response from stdin
      return new Promise<string>((resolve) => {
        let data = "";
        process.stdin.setEncoding("utf-8");
        process.stdin.on("data", (chunk) => {
          data += chunk;
          if (data.includes("\n")) {
            resolve(data.trim());
          }
        });
        process.stdin.resume();
      });
    },
    onProgress: (message) => {
      output({ type: "progress", message });
    },
    onManualCodeInput: async () => {
      output({ type: "prompt", message: "Paste the authorization code or redirect URL:" });
      return new Promise<string>((resolve) => {
        let data = "";
        process.stdin.setEncoding("utf-8");
        process.stdin.on("data", (chunk) => {
          data += chunk;
          if (data.includes("\n")) {
            resolve(data.trim());
          }
        });
        process.stdin.resume();
      });
    },
  });

  output({ type: "success", provider: providerId });
  process.exit(0);
} catch (err) {
  output({ type: "error", message: err instanceof Error ? err.message : String(err) });
  process.exit(1);
}
