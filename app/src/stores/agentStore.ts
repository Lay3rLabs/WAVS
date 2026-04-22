import { create } from 'zustand';
import {
  startAgent as cmdStartAgent,
  stopAgent as cmdStopAgent,
  agentPrompt,
  agentAbort,
  agentNewSession,
  agentGetMessages,
  agentRespondUI,
} from '../tauri/agent';

// ── Message content types (matching pi RPC protocol) ────────────────────

interface TextContent {
  type: 'text';
  text: string;
  streaming?: boolean;
}

interface ThinkingContent {
  type: 'thinking';
  thinking: string;
  streaming?: boolean;
}

interface ToolCallContent {
  type: 'toolCall';
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  streaming?: boolean;
}

type AssistantContentBlock = TextContent | ThinkingContent | ToolCallContent;

interface UserMessage {
  role: 'user';
  content: string;
  timestamp: number;
}

interface AssistantMessage {
  role: 'assistant';
  content: AssistantContentBlock[];
  model?: string;
  timestamp: number;
}

interface ToolResultMessage {
  role: 'toolResult';
  toolCallId: string;
  toolName: string;
  content: TextContent[];
  isError: boolean;
  timestamp: number;
}

interface SystemMessage {
  role: 'system';
  content: string;
  level: 'info' | 'warning' | 'error';
  timestamp: number;
}

export type AgentMessage = UserMessage | AssistantMessage | ToolResultMessage | SystemMessage;

// ── Tool execution tracking ─────────────────────────────────────────────

interface ToolExecution {
  toolCallId: string;
  toolName: string;
  args: Record<string, unknown>;
  result?: { content: Array<{ type: string; text: string }>; isError: boolean };
  status: 'running' | 'complete' | 'error';
}

// ── Pending queue (from server-side queue_update events) ─────────────────

export interface PendingQueue {
  steering: string[];
  followUp: string[];
}

// ── Pending messages (client-side, shown at bottom during streaming) ────

export interface PendingMessage {
  id: string;
  text: string;
  mode: 'steer' | 'followUp';
  timestamp: number;
}

// ── Model/usage info ────────────────────────────────────────────────────

export interface ModelInfo {
  provider: string;
  model: string;
}

export interface UsageInfo {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  contextWindow: number;
  cost: number;
}

// ── Store interface ─────────────────────────────────────────────────────

interface AgentState {
  messages: AgentMessage[];
  toolExecutions: Map<string, ToolExecution>;
  isStreaming: boolean;
  status: 'stopped' | 'running' | 'error';
  error: string | null;
  panelOpen: boolean;
  panelWidth: number;
  pendingQueue: PendingQueue;
  pendingMessages: PendingMessage[];
  pendingUIRequest: ExtensionUIRequestEvent | null;
  sessions: import('../tauri/agent').SessionInfo[];
  currentSessionId: string | null;
  modelInfo: ModelInfo | null;
  usageInfo: UsageInfo | null;

  // Actions
  sendMessage: (text: string, streamingBehavior?: 'steer' | 'followUp') => Promise<void>;
  respondToUIRequest: (id: string, response: Record<string, unknown>) => Promise<void>;
  abort: () => Promise<void>;
  newSession: () => Promise<void>;
  switchSession: (sessionPath: string) => Promise<void>;
  refreshSessions: () => Promise<void>;
  startAgent: () => Promise<void>;
  stopAgent: () => Promise<void>;
  togglePanel: () => void;
  setPanelWidth: (width: number) => void;

  // Event handlers (called from Tauri event listeners)
  handleAgentEvent: (event: AgentRpcEvent) => void;
  handleStatusEvent: (status: string, error?: string) => void;
  clearMessages: () => void;
}

// ── RPC event types ─────────────────────────────────────────────────────

interface AgentStartEvent {
  type: 'agent_start';
}

interface AgentEndEvent {
  type: 'agent_end';
  messages?: AgentMessage[];
}

interface MessageUpdateEvent {
  type: 'message_update';
  message?: unknown;
  assistantMessageEvent?: {
    type: string;
    delta?: string;
    toolCall?: { id: string; name: string; arguments?: Record<string, unknown> };
  };
}

interface ToolExecutionStartEvent {
  type: 'tool_execution_start';
  toolCallId: string;
  toolName: string;
  args: Record<string, unknown>;
}

interface ToolExecutionUpdateEvent {
  type: 'tool_execution_update';
  toolCallId: string;
  toolName: string;
  partialResult?: { content: Array<{ type: string; text: string }>; details?: unknown };
}

interface ToolExecutionEndEvent {
  type: 'tool_execution_end';
  toolCallId: string;
  toolName: string;
  result: { content: Array<{ type: string; text: string }>; details?: unknown };
  isError: boolean;
}

interface QueueUpdateEvent {
  type: 'queue_update';
  steering: string[];
  followUp: string[];
}

interface AutoRetryStartEvent {
  type: 'auto_retry_start';
  attempt: number;
  maxAttempts: number;
  delayMs: number;
  errorMessage: string;
}

interface AutoRetryEndEvent {
  type: 'auto_retry_end';
  success: boolean;
  attempt: number;
  finalError?: string;
}

interface CompactionStartEvent {
  type: 'compaction_start';
  reason: string;
}

interface CompactionEndEvent {
  type: 'compaction_end';
  reason: string;
  result?: unknown;
  aborted?: boolean;
}

interface ExtensionUIRequestEvent {
  type: 'extension_ui_request';
  id: string;
  method: string;
  // select
  title?: string;
  options?: string[];
  // confirm
  message?: string;
  // input
  placeholder?: string;
  // notify (fire-and-forget, already intercepted for __ui_control)
  notifyType?: string;
  // timeout
  timeout?: number;
}

interface ExtensionErrorEvent {
  type: 'extension_error';
  extensionPath: string;
  event: string;
  error: string;
}

type AgentRpcEvent =
  | AgentStartEvent
  | AgentEndEvent
  | MessageUpdateEvent
  | ToolExecutionStartEvent
  | ToolExecutionUpdateEvent
  | ToolExecutionEndEvent
  | QueueUpdateEvent
  | AutoRetryStartEvent
  | AutoRetryEndEvent
  | CompactionStartEvent
  | CompactionEndEvent
  | ExtensionUIRequestEvent
  | ExtensionErrorEvent
  | { type: 'message_end'; message?: Record<string, unknown> }
  | { type: 'message_start'; message?: Record<string, unknown> }
  | { type: 'turn_start' }
  | { type: 'turn_end' }
  | { type: 'session_messages'; messages: unknown[] };

// ── Helpers ─────────────────────────────────────────────────────────────

/**
 * Convert SDK-format messages (from get_messages response) to our internal AgentMessage format.
 * Also builds toolExecutions map from toolResult messages so restored sessions show tool output.
 */
function convertSdkMessages(sdkMessages: unknown[]): { messages: AgentMessage[]; toolExecutions: Map<string, ToolExecution> } {
  const result: AgentMessage[] = [];
  const toolExecutions = new Map<string, ToolExecution>();

  for (const raw of sdkMessages) {
    const msg = raw as Record<string, unknown>;
    const role = msg.role as string;

    if (role === 'user') {
      let text = '';
      if (typeof msg.content === 'string') {
        text = msg.content;
      } else if (Array.isArray(msg.content)) {
        text = (msg.content as Array<{ type: string; text?: string }>)
          .filter(b => b.type === 'text' && b.text)
          .map(b => b.text!)
          .join('\n');
      }
      result.push({ role: 'user', content: text, timestamp: (msg.timestamp as number) ?? Date.now() });
    } else if (role === 'assistant') {
      const blocks: AssistantContentBlock[] = Array.isArray(msg.content)
        ? (msg.content as Array<Record<string, unknown>>).map(block => {
            const type = block.type as string;
            if (type === 'text') return { type: 'text' as const, text: (block.text as string) ?? '', streaming: false };
            if (type === 'thinking') return { type: 'thinking' as const, thinking: (block.thinking as string) ?? '', streaming: false };
            if (type === 'toolCall') return {
              type: 'toolCall' as const,
              id: (block.id as string) ?? '',
              name: (block.name as string) ?? '',
              arguments: (block.arguments as Record<string, unknown>) ?? {},
              streaming: false,
            };
            return { type: 'text' as const, text: '', streaming: false };
          })
        : [];
      result.push({ role: 'assistant', content: blocks, timestamp: (msg.timestamp as number) ?? Date.now() });
    } else if (role === 'toolResult') {
      // Build tool execution entry so AgentToolCall can show the result
      const toolCallId = msg.toolCallId as string;
      const toolName = msg.toolName as string;
      const isError = (msg.isError as boolean) ?? false;
      const contentArr = Array.isArray(msg.content) ? msg.content as Array<{ type: string; text?: string }> : [];
      toolExecutions.set(toolCallId, {
        toolCallId,
        toolName,
        args: {},
        result: {
          content: contentArr.map(c => ({ type: c.type, text: c.text ?? '' })),
          isError,
        },
        status: isError ? 'error' : 'complete',
      });
    }
  }
  return { messages: result, toolExecutions };
}

/**
 * Get the trailing assistant message — only if no user message comes after it.
 * This ensures new turns create new assistant messages instead of appending to old ones.
 */
function getLastAssistantMessage(messages: AgentMessage[]): AssistantMessage | null {
  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    if (msg.role === 'assistant') return msg as AssistantMessage;
    if (msg.role === 'user') return null; // user message after last assistant — new turn
  }
  return null;
}

function updateLastAssistantMessage(
  messages: AgentMessage[],
  updater: (msg: AssistantMessage) => AssistantMessage,
): AgentMessage[] {
  const result = [...messages];
  for (let i = result.length - 1; i >= 0; i--) {
    const msg = result[i];
    if (msg.role === 'assistant') {
      result[i] = updater(msg as AssistantMessage);
      return result;
    }
    if (msg.role === 'user') return result; // don't cross user message boundary
  }
  return result;
}

// ── Store ───────────────────────────────────────────────────────────────

export const useAgentStore = create<AgentState>((set, get) => ({
  messages: [],
  toolExecutions: new Map(),
  isStreaming: false,
  status: 'stopped',
  error: null,
  panelOpen: false,
  panelWidth: 420,
  pendingQueue: { steering: [], followUp: [] },
  pendingMessages: [],
  pendingUIRequest: null,
  sessions: [],
  currentSessionId: null,
  modelInfo: null,
  usageInfo: null,

  respondToUIRequest: async (id: string, response: Record<string, unknown>) => {
    try {
      await agentRespondUI(id, response);
      set({ pendingUIRequest: null });
    } catch (err) {
      console.error('Failed to respond to UI request:', err);
    }
  },

  sendMessage: async (text: string, streamingBehavior?: 'steer' | 'followUp') => {
    const pendingId = streamingBehavior ? `${Date.now()}-${Math.random()}` : null;

    if (streamingBehavior) {
      // During streaming: track as pending message (shown at bottom of panel)
      // Don't add to messages array yet — that would split the streaming response
      // Converted to real user message when message_start arrives from the SDK
      set((state) => ({
        pendingMessages: [...state.pendingMessages, {
          id: pendingId!,
          text,
          mode: streamingBehavior,
          timestamp: Date.now(),
        }],
      }));
    } else {
      // Normal send (not during streaming): add to messages immediately
      const userMessage: UserMessage = {
        role: 'user',
        content: text,
        timestamp: Date.now(),
      };
      set((state) => ({ messages: [...state.messages, userMessage] }));
    }

    try {
      await agentPrompt(text, streamingBehavior);
    } catch (err) {
      console.error('Failed to send agent prompt:', err);
      // If prompt failed and it was pending, remove from pending and add as failed message
      if (pendingId) {
        set((s) => ({
          pendingMessages: s.pendingMessages.filter(p => p.id !== pendingId),
        }));
      }
      set({ error: err instanceof Error ? err.message : typeof err === "string" ? err : JSON.stringify(err) });
    }
  },

  abort: async () => {
    try {
      await agentAbort();
    } catch (err) {
      console.error('Failed to abort agent:', err);
    }
  },

  newSession: async () => {
    try {
      await agentNewSession();
      set({ messages: [], toolExecutions: new Map(), isStreaming: false, error: null, currentSessionId: null, pendingQueue: { steering: [], followUp: [] }, pendingMessages: [], usageInfo: null });
      // Refresh session list after creating a new one
      get().refreshSessions();
    } catch (err) {
      console.error('Failed to create new session:', err);
    }
  },

  switchSession: async (sessionPath: string) => {
    try {
      const { agentSwitchSession } = await import('../tauri/agent');
      await agentSwitchSession(sessionPath);
      // Find session id from path
      const session = get().sessions.find(s => s.path === sessionPath);
      // Clear current messages — session_messages event will repopulate them
      // after the relay confirms the switch and requests get_messages
      set({
        messages: [],
        toolExecutions: new Map(),
        isStreaming: false,
        error: null,
        pendingQueue: { steering: [], followUp: [] },
        pendingMessages: [],
        usageInfo: null,
        currentSessionId: session?.id ?? null,
      });
      // Refresh sessions to update modified timestamps
      get().refreshSessions();
    } catch (err) {
      console.error('Failed to switch session:', err);
    }
  },

  refreshSessions: async () => {
    try {
      const { agentListSessions } = await import('../tauri/agent');
      const sessions = await agentListSessions();
      set({ sessions });
    } catch (err) {
      console.error('Failed to list sessions:', err);
    }
  },

  startAgent: async () => {
    try {
      await cmdStartAgent();
      set({ status: 'running', error: null });
      // Refresh session list
      get().refreshSessions();
      // Request messages from the continued session (sidecar auto-continues most recent)
      setTimeout(async () => {
        try {
          await agentGetMessages();
        } catch {
          // May fail if agent not ready yet
        }
      }, 1500);
    } catch (err: unknown) {
      console.error('Failed to start agent:', err);
      const message = err instanceof Error ? err.message
        : typeof err === 'object' && err !== null && 'message' in err ? String((err as { message: unknown }).message)
        : typeof err === 'string' ? err
        : JSON.stringify(err);
      set({ status: 'error', error: message });
    }
  },

  stopAgent: async () => {
    try {
      await cmdStopAgent();
      set({ status: 'stopped', isStreaming: false });
    } catch (err) {
      console.error('Failed to stop agent:', err);
    }
  },

  togglePanel: () => set((state) => ({ panelOpen: !state.panelOpen })),

  setPanelWidth: (width: number) => set({ panelWidth: width }),

  clearMessages: () => set({ messages: [], toolExecutions: new Map(), pendingQueue: { steering: [], followUp: [] }, pendingMessages: [], usageInfo: null }),

  handleStatusEvent: (status: string, error?: string) => {
    const mappedStatus = status === 'running' ? 'running'
      : status === 'error' ? 'error'
      : 'stopped';
    console.log('[Agent] handleStatusEvent:', status, '→', mappedStatus);
    set({ status: mappedStatus, error: error ?? null });
  },

  handleAgentEvent: (event: AgentRpcEvent) => {
    const state = get();

    // Debug: log all event types
    if (event.type !== 'message_update') {
      console.log('[Agent] event:', event.type, 'toolCallId' in event ? (event as any).toolCallId : '');
    }

    switch (event.type) {
      case 'agent_start': {
        set({ isStreaming: true });
        break;
      }

      case 'agent_end': {
        set({ isStreaming: false });
        break;
      }

      case 'message_start': {
        // When the SDK picks up a queued user message, convert it from pending to a real message
        const msg = (event as unknown as { message?: Record<string, unknown> }).message;
        if (msg && msg.role === 'user') {
          const content = msg.content;
          let text = '';
          if (typeof content === 'string') {
            text = content;
          } else if (Array.isArray(content)) {
            text = (content as Array<{ type: string; text?: string }>)
              .filter(b => b.type === 'text' && b.text)
              .map(b => b.text!)
              .join('\n');
          }

          const pending = get().pendingMessages;
          // Match by exact text first, fall back to trimmed comparison
          const matchIdx = pending.findIndex(p => p.text === text || p.text.trim() === text.trim());
          if (matchIdx !== -1) {
            // Convert pending → real user message
            const matched = pending[matchIdx];
            const userMessage: UserMessage = {
              role: 'user',
              content: matched.text,
              timestamp: matched.timestamp,
            };
            set((s) => ({
              messages: [...s.messages, userMessage],
              pendingMessages: s.pendingMessages.filter((_, i) => i !== matchIdx),
            }));
          } else {
            // Not from pending — already added by sendMessage(), skip to avoid duplicates
          }
        }
        break;
      }

      case 'message_end': {
        // Extract model/provider and usage from the assistant message
        const msg = (event as unknown as { message?: Record<string, unknown> }).message;
        if (msg && msg.role === 'assistant') {
          const provider = msg.provider as string | undefined;
          const model = msg.model as string | undefined;
          if (provider && model) {
            set({ modelInfo: { provider, model } });
          }
          const usage = msg.usage as Record<string, unknown> | undefined;
          if (usage) {
            const prev = get().usageInfo;
            // inputTokens represents current context size (what was sent to the LLM this turn)
            // cost accumulates across turns
            const inputTokens = (usage.input as number) ?? 0;
            const cacheRead = (usage.cacheRead as number) ?? 0;
            const cacheWrite = (usage.cacheWrite as number) ?? 0;
            set({
              usageInfo: {
                inputTokens,
                outputTokens: (usage.output as number) ?? 0,
                totalTokens: inputTokens + cacheRead + cacheWrite,
                cacheReadTokens: cacheRead,
                cacheWriteTokens: cacheWrite,
                contextWindow: prev?.contextWindow ?? 200000,
                cost: (prev?.cost ?? 0) + ((usage.cost as Record<string, number>)?.total ?? 0),
              },
            });
          }
        }
        break;
      }

      case 'session_messages': {
        // Received when switching sessions — rebuild messages from SDK format
        const sdkMessages = (event as unknown as { messages: unknown[] }).messages;
        if (!Array.isArray(sdkMessages)) break;
        const { messages: converted, toolExecutions } = convertSdkMessages(sdkMessages);
        set({ messages: converted, toolExecutions, isStreaming: false });
        break;
      }

      case 'message_update': {
        const ame = event.assistantMessageEvent;
        if (!ame) break;

        if (ame.type.startsWith('toolcall')) {
          console.log('[Agent] message_update ame:', ame.type, JSON.stringify(ame).slice(0, 300));
        }

        switch (ame.type) {
          case 'text_delta': {
            const lastMsg = getLastAssistantMessage(state.messages);
            if (!lastMsg) {
              const newMsg: AssistantMessage = {
                role: 'assistant',
                content: [{ type: 'text', text: ame.delta ?? '', streaming: true }],
                timestamp: Date.now(),
              };
              set({ messages: [...state.messages, newMsg] });
            } else {
              set({
                messages: updateLastAssistantMessage(state.messages, (msg) => {
                  const content = [...msg.content];
                  const lastBlock = content[content.length - 1];
                  if (lastBlock && lastBlock.type === 'text') {
                    content[content.length - 1] = {
                      ...lastBlock,
                      text: lastBlock.text + (ame.delta ?? ''),
                      streaming: true,
                    };
                  } else {
                    content.push({ type: 'text', text: ame.delta ?? '', streaming: true });
                  }
                  return { ...msg, content };
                }),
              });
            }
            break;
          }

          case 'text_end': {
            // Mark the last text block as no longer streaming
            set({
              messages: updateLastAssistantMessage(state.messages, (msg) => {
                const content = [...msg.content];
                for (let i = content.length - 1; i >= 0; i--) {
                  if (content[i].type === 'text') {
                    content[i] = { ...content[i], streaming: false };
                    break;
                  }
                }
                return { ...msg, content };
              }),
            });
            break;
          }

          case 'thinking_delta': {
            const lastMsg = getLastAssistantMessage(state.messages);
            if (!lastMsg) {
              const newMsg: AssistantMessage = {
                role: 'assistant',
                content: [{ type: 'thinking', thinking: ame.delta ?? '', streaming: true }],
                timestamp: Date.now(),
              };
              set({ messages: [...state.messages, newMsg] });
            } else {
              set({
                messages: updateLastAssistantMessage(state.messages, (msg) => {
                  const content = [...msg.content];
                  const lastBlock = content[content.length - 1];
                  if (lastBlock && lastBlock.type === 'thinking') {
                    content[content.length - 1] = {
                      ...lastBlock,
                      thinking: lastBlock.thinking + (ame.delta ?? ''),
                      streaming: true,
                    };
                  } else {
                    content.push({ type: 'thinking', thinking: ame.delta ?? '', streaming: true });
                  }
                  return { ...msg, content };
                }),
              });
            }
            break;
          }

          case 'thinking_end': {
            // Mark the last thinking block as no longer streaming
            set({
              messages: updateLastAssistantMessage(state.messages, (msg) => {
                const content = [...msg.content];
                for (let i = content.length - 1; i >= 0; i--) {
                  if (content[i].type === 'thinking') {
                    content[i] = { ...content[i], streaming: false };
                    break;
                  }
                }
                return { ...msg, content };
              }),
            });
            break;
          }

          case 'toolcall_start': {
            // toolcall_start has no toolCall field — extract from partial.content[contentIndex]
            const contentIndex = (ame as unknown as { contentIndex: number }).contentIndex;
            const partial = (ame as unknown as { partial: { content: Array<Record<string, unknown>> } }).partial;
            const partialBlock = partial?.content?.[contentIndex];
            const tcId = (partialBlock?.id as string) ?? `pending-${contentIndex}`;
            const tcName = (partialBlock?.name as string) ?? 'unknown';

            const lastMsg = getLastAssistantMessage(state.messages);
            const toolCallBlock: ToolCallContent = {
              type: 'toolCall',
              id: tcId,
              name: tcName,
              arguments: {},
              streaming: true,
            };

            if (!lastMsg) {
              const newMsg: AssistantMessage = {
                role: 'assistant',
                content: [toolCallBlock],
                timestamp: Date.now(),
              };
              set({ messages: [...state.messages, newMsg] });
            } else {
              set({
                messages: updateLastAssistantMessage(state.messages, (msg) => ({
                  ...msg,
                  content: [...msg.content, toolCallBlock],
                })),
              });
            }
            break;
          }

          case 'toolcall_delta': {
            // toolcall_delta has delta (arg text chunk) — append to last streaming toolCall block
            const delta = ame.delta ?? '';
            if (!delta) break;
            set({
              messages: updateLastAssistantMessage(state.messages, (msg) => {
                const content = [...msg.content];
                // Find last streaming toolCall block
                for (let i = content.length - 1; i >= 0; i--) {
                  const block = content[i];
                  if (block.type === 'toolCall' && block.streaming) {
                    content[i] = {
                      ...block,
                      _rawArgs: ((block as unknown as { _rawArgs?: string })._rawArgs ?? '') + delta,
                    } as unknown as AssistantContentBlock;
                    break;
                  }
                }
                return { ...msg, content };
              }),
            });
            break;
          }

          case 'toolcall_end': {
            const tc = ame.toolCall;
            if (!tc) break;
            // Update the existing tool call block with final id, arguments, and mark complete
            const contentIndex = (ame as unknown as { contentIndex: number }).contentIndex;
            set({
              messages: updateLastAssistantMessage(state.messages, (msg) => {
                let toolCallCount = 0;
                const content = msg.content.map((block) => {
                  if (block.type === 'toolCall') {
                    // Match by id if available, or by being the pending block
                    if (block.id === tc.id || block.id === `pending-${contentIndex}`) {
                      return { ...block, id: tc.id, name: tc.name, arguments: tc.arguments ?? block.arguments, streaming: false };
                    }
                    toolCallCount++;
                  }
                  return block;
                });
                return { ...msg, content };
              }),
            });
            break;
          }

          case 'error': {
            // LLM error (aborted, API error, etc.)
            const reason = (ame as { reason?: string }).reason ?? 'unknown';
            const sysMsg: SystemMessage = {
              role: 'system',
              content: reason === 'aborted' ? 'Agent was aborted.' : `Agent error: ${reason}`,
              level: reason === 'aborted' ? 'info' : 'error',
              timestamp: Date.now(),
            };
            set((s) => ({ messages: [...s.messages, sysMsg] }));
            break;
          }
        }
        break;
      }

      case 'tool_execution_start': {
        console.log('[Agent] tool_execution_start:', event.toolCallId, event.toolName);
        const newExecutions = new Map(state.toolExecutions);
        newExecutions.set(event.toolCallId, {
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          args: event.args,
          status: 'running',
        });
        set({ toolExecutions: newExecutions });
        break;
      }

      case 'tool_execution_update': {
        // Partial tool output — update the execution entry
        const newExecutions = new Map(state.toolExecutions);
        const existing = newExecutions.get(event.toolCallId);
        if (existing && event.partialResult?.content) {
          newExecutions.set(event.toolCallId, {
            ...existing,
            result: { content: event.partialResult.content, isError: false },
          });
          set({ toolExecutions: newExecutions });
        }
        break;
      }

      case 'tool_execution_end': {
        console.log('[Agent] tool_execution_end:', event.toolCallId, event.toolName, 'error:', event.isError);
        const newExecutions = new Map(state.toolExecutions);
        newExecutions.set(event.toolCallId, {
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          args: (newExecutions.get(event.toolCallId)?.args ?? {}),
          result: { content: event.result.content, isError: event.isError },
          status: event.isError ? 'error' : 'complete',
        });
        set({ toolExecutions: newExecutions });

        // Add a tool result message
        const toolResultMsg: ToolResultMessage = {
          role: 'toolResult',
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          content: event.result.content
            .filter((c) => c.type === 'text')
            .map((c) => ({ type: 'text' as const, text: c.text })),
          isError: event.isError,
          timestamp: Date.now(),
        };
        set((s) => ({ messages: [...s.messages, toolResultMsg] }));
        break;
      }

      case 'queue_update': {
        set({
          pendingQueue: {
            steering: event.steering,
            followUp: event.followUp,
          },
        });
        break;
      }

      case 'auto_retry_start': {
        const sysMsg: SystemMessage = {
          role: 'system',
          content: `Retrying (attempt ${event.attempt}/${event.maxAttempts}, ${Math.round(event.delayMs / 1000)}s delay): ${event.errorMessage}`,
          level: 'warning',
          timestamp: Date.now(),
        };
        set((s) => ({ messages: [...s.messages, sysMsg] }));
        break;
      }

      case 'auto_retry_end': {
        if (!event.success && event.finalError) {
          const sysMsg: SystemMessage = {
            role: 'system',
            content: `Retry failed: ${event.finalError}`,
            level: 'error',
            timestamp: Date.now(),
          };
          set((s) => ({ messages: [...s.messages, sysMsg] }));
        }
        break;
      }

      case 'compaction_start': {
        const sysMsg: SystemMessage = {
          role: 'system',
          content: 'Compacting conversation context…',
          level: 'info',
          timestamp: Date.now(),
        };
        set((s) => ({ messages: [...s.messages, sysMsg] }));
        break;
      }

      case 'compaction_end': {
        if (event.aborted) {
          const sysMsg: SystemMessage = {
            role: 'system',
            content: 'Compaction was aborted.',
            level: 'warning',
            timestamp: Date.now(),
          };
          set((s) => ({ messages: [...s.messages, sysMsg] }));
        }
        break;
      }

      case 'extension_ui_request': {
        // Dialog methods (select, confirm, input, editor) need a response
        const dialogMethods = ['select', 'confirm', 'input', 'editor'];
        if (dialogMethods.includes(event.method)) {
          set({ pendingUIRequest: event });
        }
        // Fire-and-forget methods (notify, setStatus, etc.) are handled
        // by the Rust relay (__ui_control: interception) or ignored
        break;
      }

      case 'extension_error': {
        const sysMsg: SystemMessage = {
          role: 'system',
          content: `Extension error: ${event.error}`,
          level: 'error',
          timestamp: Date.now(),
        };
        set((s) => ({ messages: [...s.messages, sysMsg] }));
        break;
      }
    }
  },
}));
