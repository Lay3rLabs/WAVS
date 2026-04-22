import { invoke } from '@tauri-apps/api/core';

export async function startAgent(): Promise<void> {
  return invoke<void>('cmd_start_agent');
}

export async function stopAgent(): Promise<void> {
  return invoke<void>('cmd_stop_agent');
}

export async function agentPrompt(message: string, streamingBehavior?: 'steer' | 'followUp'): Promise<void> {
  return invoke<void>('cmd_agent_prompt', { message, streaming_behavior: streamingBehavior ?? null });
}

export async function agentAbort(): Promise<void> {
  return invoke<void>('cmd_agent_abort');
}

export async function agentStatus(): Promise<{ status: string; error?: string }> {
  return invoke('cmd_agent_status');
}

export async function agentNewSession(): Promise<void> {
  return invoke<void>('cmd_agent_new_session');
}

export async function agentSetModel(provider: string, modelId: string): Promise<void> {
  return invoke<void>('cmd_agent_set_model', { provider, model_id: modelId });
}

export async function agentSetThinking(level: string): Promise<void> {
  return invoke<void>('cmd_agent_set_thinking', { level });
}

export async function agentGetMessages(): Promise<void> {
  return invoke<void>('cmd_agent_get_messages');
}

/**
 * Send an extension UI response back to the agent.
 * Used when the agent sends an extension_ui_request (select, confirm, input).
 */
export async function agentRespondUI(id: string, response: Record<string, unknown>): Promise<void> {
  return invoke<void>('cmd_agent_respond_ui', { id, response });
}

// ── Auth Management ─────────────────────────────────────────────────────

export interface AgentAuthInfo {
  type: 'api_key' | 'oauth' | string;
  configured: boolean;
  masked_key?: string;
  expires?: number;
}

/** Get configured auth providers (never exposes raw keys). */
export async function agentGetAuth(): Promise<Record<string, AgentAuthInfo>> {
  return invoke<Record<string, AgentAuthInfo>>('cmd_agent_get_auth');
}

/** Set an API key for a provider. */
export async function agentSetApiKey(provider: string, apiKey: string): Promise<void> {
  return invoke<void>('cmd_agent_set_api_key', { provider, api_key: apiKey });
}

/** Set OAuth credentials for a provider. */
export async function agentSetOauth(provider: string, refresh: string, access: string, expires: number): Promise<void> {
  return invoke<void>('cmd_agent_set_oauth', { provider, refresh, access, expires });
}

/** Remove credentials for a provider. */
export async function agentRemoveAuth(provider: string): Promise<void> {
  return invoke<void>('cmd_agent_remove_auth', { provider });
}

/** Start an OAuth login flow for a provider. Events emitted on agent:oauth. */
export async function agentOAuthLogin(provider: string): Promise<void> {
  return invoke<void>('cmd_agent_oauth_login', { provider });
}

/**
 * Save agent settings to the settings store.
 * Uses the existing settings save infrastructure — we just update the
 * agent-specific fields via the general settings update command.
 */
// ── Sessions ────────────────────────────────────────────────────────────

export interface SessionInfo {
  id: string;
  path: string;
  created: string;    // ISO 8601
  modified: string;   // ISO 8601
  message_count: number;
  first_message: string;
  name: string | null;
}

/** List all saved agent sessions, sorted by modified desc. */
export async function agentListSessions(): Promise<SessionInfo[]> {
  return invoke<SessionInfo[]>('cmd_agent_list_sessions');
}

/** Switch the agent to a different session. */
export async function agentSwitchSession(sessionPath: string): Promise<void> {
  return invoke<void>('cmd_agent_switch_session', { session_path: sessionPath });
}

// ── Settings ────────────────────────────────────────────────────────────

export async function saveAgentSettings(updates: {
  agent_model_provider?: string | null;
  agent_model_id?: string | null;
  agent_thinking_level?: string | null;
  agent_base_url?: string | null;
  agent_auto_start?: boolean;
  agent_panel_width?: number | null;
}): Promise<void> {
  return invoke<void>('cmd_save_agent_settings', { updates });
}
