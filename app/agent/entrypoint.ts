/**
 * WAVS Agent Sidecar — Pi coding agent in RPC mode.
 *
 * Spawned by Tauri as a child process, communicates over stdin/stdout JSON lines.
 * Uses the pi SDK directly (not the CLI).
 *
 * Env vars:
 *   WAVS_URL          — WAVS node URL (e.g. http://localhost:8080)
 *   WAVS_MCP_TOKEN    — Auth token for wavs-mcp
 *   WAVS_HOME         — WAVS project home directory (reference for system prompt)
 *   WAVS_AGENT_WORKSPACE — Agent workspace directory (cwd for coding tools)
 *   WAVS_AUTH_DIR     — Directory for auth.json credential storage
 */

import path from "node:path";
import { fileURLToPath } from "node:url";
import { mkdirSync, existsSync, readFileSync } from "node:fs";
import { getModel } from "@mariozechner/pi-ai";
import {
  AuthStorage,
  type CreateAgentSessionRuntimeFactory,
  createAgentSessionFromServices,
  createAgentSessionRuntime,
  createAgentSessionServices,
  createCodingTools,
  ModelRegistry,
  runRpcMode,
  SessionManager,
  SettingsManager,
} from "@mariozechner/pi-coding-agent";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// --- Environment ---
const wavsUrl = process.env.WAVS_URL ?? "http://localhost:8080";
const mcpToken = process.env.WAVS_MCP_TOKEN ?? "";
const wavsHome = process.env.WAVS_HOME ?? process.cwd();
const workspace = process.env.WAVS_AGENT_WORKSPACE ?? wavsHome;
const authDir = process.env.WAVS_AUTH_DIR;
if (!authDir) {
  console.error("WAVS_AUTH_DIR is required");
  process.exit(1);
}

// --- Auth & Models ---
const authStorage = AuthStorage.create(path.join(authDir, "auth.json"));
const modelRegistry = ModelRegistry.inMemory(authStorage);

// Default model — read from settings.json if available, fall back to Anthropic
let savedProvider = "anthropic";
let savedModelId = "claude-sonnet-4-20250514";
try {
  const settingsPath = path.join(authDir, "settings.json");
  if (existsSync(settingsPath)) {
    const saved = JSON.parse(readFileSync(settingsPath, "utf-8"));
    if (saved.agent_model_provider) savedProvider = saved.agent_model_provider;
    if (saved.agent_model_id) savedModelId = saved.agent_model_id;
  }
} catch {
  // Use defaults on any read/parse error
}
const defaultModel = modelRegistry.find(savedProvider, savedModelId)
  ?? getModel("anthropic", "claude-sonnet-4-20250514");

// --- System Prompt ---
const systemPrompt = `You are the WAVS Developer Assistant, an expert AI embedded in the WAVS desktop application.

You help developers build, deploy, and manage WebAssembly-based Actively Validated Services (AVS).

## Capabilities
- **Coding tools**: Read, write, edit files and run bash commands in the WAVS project
- **WAVS tools**: List services, deploy, query logs, execute components, manage the node — all via wavs-mcp
- **UI control**: Navigate the app, show toasts, open service details

## Behavioral Guidelines
- After deploying or modifying a service, call \`ui_navigate\` to show the result
- After errors, check \`wavs_query_logs\` or \`wavs_query_component_logs\` for details
- Use compact, actionable responses
- When building WASM components, use \`cargo component build --release\` and check checksums
- For multi-step operations (deploy, update), follow the standard flows step by step
- Use \`wavs_list_services\`, \`wavs_node_health\`, etc. to get current state — don't assume it

## Environment
- WAVS Node URL: ${wavsUrl}
- WAVS Home: ${wavsHome} (node configuration directory — contains wavs.toml and related config)
- Agent Workspace: ${workspace} (your working directory for creating/editing files)`;

// --- Settings ---
const settingsManager = SettingsManager.inMemory({
  compaction: { enabled: true },
  retry: { enabled: true, maxRetries: 2 },
});

// --- Extension Paths ---
const extensionPaths = [
  path.join(__dirname, "extensions", "wavs-tools.ts"),
  path.join(__dirname, "extensions", "ui-control.ts"),
];

// --- Create Runtime ---
// Ensure workspace exists
if (!existsSync(workspace)) {
  mkdirSync(workspace, { recursive: true });
}
const cwd = workspace;

const createRuntime: CreateAgentSessionRuntimeFactory = async ({
  cwd: runtimeCwd,
  sessionManager,
  sessionStartEvent,
}) => {
  const services = await createAgentSessionServices({
    cwd: runtimeCwd,
    agentDir: authDir,
    authStorage,
    modelRegistry,
    settingsManager,
    resourceLoaderOptions: {
      noSkills: true,
      noPromptTemplates: true,
      noThemes: true,
      // Only load our bundled extensions, not user/project extensions
      noExtensions: true,
      additionalExtensionPaths: extensionPaths,
      systemPrompt,
      // Don't pick up AGENTS.md from cwd or agentDir
      agentsFilesOverride: () => ({ agentsFiles: [] }),
    },
  });

  return {
    ...(await createAgentSessionFromServices({
      services,
      sessionManager,
      sessionStartEvent,
      model: defaultModel ?? undefined,
      thinkingLevel: "low",
      tools: createCodingTools(runtimeCwd),
    })),
    services,
    diagnostics: services.diagnostics,
  };
};

// Persist sessions to disk under <authDir>/sessions/
// Auto-continue the most recent session if one exists.
const sessionsDir = path.join(authDir, "sessions");
const sessionManager = SessionManager.continueRecent(cwd, sessionsDir);

const runtime = await createAgentSessionRuntime(createRuntime, {
  cwd,
  agentDir: authDir,
  sessionManager,
});

// --- Enter RPC Mode ---
await runRpcMode(runtime);
