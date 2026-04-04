/**
 * ui-control extension — Tools for controlling the Tauri frontend.
 *
 * Registers tools that send commands back through the RPC channel to the Tauri backend.
 * Commands are encoded as `ctx.ui.notify()` calls with a `__ui_control:` prefix
 * followed by a JSON payload. The Tauri sidecar manager intercepts these and emits
 * them as Tauri events to the React frontend.
 */

import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";
import { Type } from "@sinclair/typebox";
import { StringEnum } from "@mariozechner/pi-ai";

/**
 * Send a UI control command via the notify mechanism.
 * The Tauri backend intercepts messages starting with `__ui_control:` and
 * dispatches them as frontend events.
 */
function sendUiControl(ctx: { ui: { notify(message: string, type?: string): void } }, command: Record<string, unknown>): void {
  ctx.ui.notify(`__ui_control:${JSON.stringify(command)}`, "info");
}

export default function uiControl(pi: ExtensionAPI) {
  // --- ui_navigate ---
  pi.registerTool({
    name: "ui_navigate",
    label: "Navigate UI",
    description:
      "Navigate the WAVS desktop app to a specific page.\n" +
      "Available routes:\n" +
      "  /services                              — Service list\n" +
      "  /services/{chain}/{address}             — Service detail (e.g. /services/evm:31337/0xABC...)\n" +
      "  /services/{chain}/{address}/edit         — Edit a service\n" +
      "  /services/new                           — Create new service\n" +
      "  /components                             — Uploaded components\n" +
      "  /activity                               — Triggers & submissions activity\n" +
      "  /logs                                   — Node logs\n" +
      "  /health                                 — Node health\n" +
      "  /settings                               — App settings\n" +
      "To open a specific service, use /services/{chain}/{address} where chain and address come from the service's manager field.",
    parameters: Type.Object({
      path: Type.String({ description: "The route path to navigate to" }),
    }),

    async execute(toolCallId, params, signal, onUpdate, ctx) {
      sendUiControl(ctx, { action: "navigate", path: params.path });

      return {
        content: [{ type: "text", text: `Navigated to ${params.path}` }],
        details: { action: "navigate", path: params.path },
      };
    },
  });

  // --- ui_toast ---
  pi.registerTool({
    name: "ui_toast",
    label: "Show Toast",
    description:
      "Show a toast notification in the WAVS desktop app. " +
      "Use to inform the user about completed actions, warnings, or errors.",
    parameters: Type.Object({
      message: Type.String({ description: "The toast message to display" }),
      level: StringEnum(["success", "error", "info", "warning"] as const, {
        description: "Toast severity level",
      }),
    }),

    async execute(toolCallId, params, signal, onUpdate, ctx) {
      sendUiControl(ctx, { action: "toast", message: params.message, level: params.level });

      return {
        content: [{ type: "text", text: `Showed ${params.level} toast: ${params.message}` }],
        details: { action: "toast", message: params.message, level: params.level },
      };
    },
  });

  // --- ui_copy_to_clipboard ---
  pi.registerTool({
    name: "ui_copy_to_clipboard",
    label: "Copy to Clipboard",
    description:
      "Copy text to the user's clipboard. Use to share addresses, commands, config snippets, or any text the user might want to paste elsewhere. IMPORTANT: Only use when the user explicitly asks to copy something — never copy to clipboard unprompted, as it overwrites existing clipboard contents.",
    parameters: Type.Object({
      text: Type.String({ description: "The text to copy to the clipboard" }),
    }),

    async execute(toolCallId, params, signal, onUpdate, ctx) {
      sendUiControl(ctx, { action: "copy_to_clipboard", text: params.text });

      return {
        content: [{ type: "text", text: `Copied to clipboard` }],
        details: { action: "copy_to_clipboard", text: params.text },
      };
    },
  });


}
