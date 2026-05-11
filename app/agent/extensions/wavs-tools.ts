/**
 * wavs-tools extension — Bridges wavs-mcp tools into pi.
 *
 * Spawns wavs-mcp as a child process and communicates via MCP (JSON-RPC 2.0 over stdio).
 * All tools from wavs-mcp are dynamically registered in pi, including tools that
 * appear/disappear at runtime (e.g. wavs_exec_* when services are deployed/removed).
 */

import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { Type, type TSchema } from "@sinclair/typebox";
import { spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import fs from "node:fs";
import { createInterface } from "node:readline";

// --- MCP Protocol Types ---

interface McpTool {
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
}

interface McpJsonRpcRequest {
  jsonrpc: "2.0";
  id?: number;
  method: string;
  params?: Record<string, unknown>;
}

interface McpJsonRpcResponse {
  jsonrpc: "2.0";
  id?: number;
  method?: string;
  result?: Record<string, unknown>;
  error?: { code: number; message: string; data?: unknown };
}

interface McpToolCallResult {
  content: Array<{ type: string; text?: string }>;
  isError?: boolean;
}

// --- JSON Schema to TypeBox conversion ---

function jsonSchemaToTypeBox(schema: Record<string, unknown>): TSchema {
  if (!schema || typeof schema !== "object") {
    return Type.Any();
  }

  const type = schema.type as string | undefined;

  switch (type) {
    case "object": {
      const properties = (schema.properties ?? {}) as Record<string, Record<string, unknown>>;
      const required = (schema.required ?? []) as string[];
      const props: Record<string, TSchema> = {};

      for (const [key, propSchema] of Object.entries(properties)) {
        const converted = jsonSchemaToTypeBox(propSchema);
        props[key] = required.includes(key) ? converted : Type.Optional(converted);
      }

      return Type.Object(props);
    }
    case "array": {
      const items = schema.items as Record<string, unknown> | undefined;
      return Type.Array(items ? jsonSchemaToTypeBox(items) : Type.Any());
    }
    case "string":
      if (schema.enum) {
        return Type.Union((schema.enum as string[]).map((v) => Type.Literal(v)));
      }
      return Type.String(schema.description ? { description: schema.description as string } : {});
    case "number":
    case "integer":
      return Type.Number(schema.description ? { description: schema.description as string } : {});
    case "boolean":
      return Type.Boolean(schema.description ? { description: schema.description as string } : {});
    default:
      return Type.Any();
  }
}

// --- MCP Client ---

class McpClient {
  private child: ChildProcess | null = null;
  private nextId = 1;
  private pending = new Map<number, { resolve: (v: McpJsonRpcResponse) => void; reject: (e: Error) => void }>();
  private onNotification: ((method: string, params?: Record<string, unknown>) => void) | null = null;

  constructor(private readonly binaryPath: string) {}

  async start(args: string[]): Promise<void> {
    this.child = spawn(this.binaryPath, args, {
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env },
    });

    // Log stderr for debugging
    if (this.child.stderr) {
      this.child.stderr.on("data", (chunk: Buffer) => {
        console.error(`[wavs-mcp stderr] ${chunk.toString().trimEnd()}`);
      });
    }

    // Wrap in a promise so spawn errors reject instead of crashing the process
    await new Promise<void>((resolve, reject) => {
      let settled = false;

      this.child!.on("error", (err) => {
        this.child = null;
        if (!settled) {
          settled = true;
          reject(new Error(`Failed to spawn wavs-mcp: ${err.message}`));
        }
      });

      if (!this.child!.stdout || !this.child!.stdin) {
        this.child = null;
        reject(new Error("Failed to spawn wavs-mcp: no stdio"));
        return;
      }

      // Process started successfully — resolve immediately, wire up readers
      settled = true;
      resolve();
    });

    const rl = createInterface({ input: this.child!.stdout! });

    rl.on("line", (line) => {
      try {
        const msg = JSON.parse(line) as McpJsonRpcResponse;

        // Notification (no id)
        if (msg.id === undefined && msg.method) {
          this.onNotification?.(msg.method, msg.result);
          return;
        }

        // Response to a request
        if (msg.id !== undefined) {
          const p = this.pending.get(msg.id);
          if (p) {
            this.pending.delete(msg.id);
            p.resolve(msg);
          }
        }
      } catch {
        // Ignore non-JSON lines (e.g. stderr leaking to stdout)
      }
    });

    this.child!.on("exit", (code) => {
      console.error(`[wavs-mcp] Process exited with code ${code}`);
      // Reject all pending requests
      for (const [, p] of this.pending) {
        p.reject(new Error(`wavs-mcp exited with code ${code}`));
      }
      this.pending.clear();
      this.child = null;
    });
  }

  setNotificationHandler(handler: (method: string, params?: Record<string, unknown>) => void): void {
    this.onNotification = handler;
  }

  async request(method: string, params?: Record<string, unknown>): Promise<McpJsonRpcResponse> {
    if (!this.child?.stdin) {
      throw new Error("wavs-mcp not running");
    }

    const id = this.nextId++;
    const req: McpJsonRpcRequest = { jsonrpc: "2.0", id, method, params };

    return new Promise<McpJsonRpcResponse>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.child!.stdin!.write(JSON.stringify(req) + "\n");
    });
  }

  notify(method: string, params?: Record<string, unknown>): void {
    if (!this.child?.stdin) return;
    const msg: McpJsonRpcRequest = { jsonrpc: "2.0", method, params };
    this.child.stdin.write(JSON.stringify(msg) + "\n");
  }

  kill(): void {
    if (this.child) {
      this.child.kill("SIGTERM");
      this.child = null;
    }
  }

  get alive(): boolean {
    return this.child !== null;
  }
}

// --- Find wavs-mcp binary ---

function findMcpBinary(): string {
  // 1. Explicit env var
  if (process.env.WAVS_MCP_BINARY) {
    return process.env.WAVS_MCP_BINARY;
  }

  // 2. Search common build output locations
  const wavsHome = process.env.WAVS_HOME ?? process.cwd();
  const candidates = [
    path.join(wavsHome, "target", "release", "wavs-mcp"),
    path.join(wavsHome, "target", "debug", "wavs-mcp"),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  // 3. Fall back to PATH
  return "wavs-mcp";
}

// --- Extension ---

export default function wavsTools(pi: ExtensionAPI) {
  let mcpClient: McpClient | null = null;
  const registeredToolNames = new Set<string>();

  async function registerMcpTools(): Promise<void> {
    if (!mcpClient?.alive) return;

    const resp = await mcpClient.request("tools/list");
    if (resp.error) {
      console.error("[wavs-tools] tools/list error:", resp.error.message);
      return;
    }

    const tools = ((resp.result as Record<string, unknown>)?.tools ?? []) as McpTool[];

    // Track which tools are new vs existing
    const newToolNames = new Set(tools.map((t) => t.name));

    // Note: pi doesn't have an unregisterTool API, so we just re-register.
    // Tools with the same name will override previous registrations.

    for (const tool of tools) {
      const schema = tool.inputSchema
        ? jsonSchemaToTypeBox(tool.inputSchema as Record<string, unknown>)
        : Type.Object({});

      pi.registerTool({
        name: tool.name,
        label: tool.name,
        description: tool.description ?? `WAVS MCP tool: ${tool.name}`,
        parameters: schema,

        async execute(toolCallId, params, signal, onUpdate, ctx) {
          if (!mcpClient?.alive) {
            return {
              content: [{ type: "text", text: "Error: wavs-mcp is not running" }],
              details: {},
            };
          }

          try {
            const resp = await mcpClient.request("tools/call", {
              name: tool.name,
              arguments: params,
            });

            if (resp.error) {
              return {
                content: [{ type: "text", text: `MCP error: ${resp.error.message}` }],
                details: { error: resp.error },
              };
            }

            const result = resp.result as unknown as McpToolCallResult;
            const textParts = (result.content ?? [])
              .filter((c) => c.type === "text" && c.text)
              .map((c) => c.text!);

            return {
              content: [{ type: "text", text: textParts.join("\n") || "(empty result)" }],
              details: { mcpResult: result },
            };
          } catch (err) {
            return {
              content: [{ type: "text", text: `Error calling ${tool.name}: ${err}` }],
              details: {},
            };
          }
        },
      });

      registeredToolNames.add(tool.name);
    }

    // Log registered tools
    const count = newToolNames.size;
    console.error(`[wavs-tools] Registered ${count} MCP tool(s): ${[...newToolNames].join(", ")}`);
  }

  pi.on("session_start", async (_event, ctx) => {
    const binaryPath = findMcpBinary();
    const wavsUrl = process.env.WAVS_URL ?? "http://localhost:8080";
    const mcpToken = process.env.WAVS_MCP_TOKEN ?? "";

    mcpClient = new McpClient(binaryPath);

    try {
      const args = ["--wavs-url", wavsUrl];
      if (mcpToken) {
        args.push("--token", mcpToken);
      }
      args.push("--exec-enabled");

      await mcpClient.start(args);

      // MCP Initialize handshake
      const initResp = await mcpClient.request("initialize", {
        protocolVersion: "2024-11-05",
        capabilities: {},
        clientInfo: { name: "wavs-agent", version: "1.0.0" },
      });

      if (initResp.error) {
        console.error("[wavs-tools] MCP initialize error:", initResp.error.message);
        return;
      }

      // Send initialized notification
      mcpClient.notify("notifications/initialized");

      // Listen for tool list changes
      mcpClient.setNotificationHandler((method) => {
        if (method === "notifications/tools/list_changed") {
          registerMcpTools().catch((err) => {
            console.error("[wavs-tools] Failed to re-register tools:", err);
          });
        }
      });

      // Initial tool registration
      await registerMcpTools();
    } catch (err) {
      console.error("[wavs-tools] Failed to start wavs-mcp:", err);
    }
  });

  pi.on("session_shutdown", async () => {
    mcpClient?.kill();
    mcpClient = null;
  });
}
