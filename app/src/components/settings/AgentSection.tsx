import { useState, useEffect } from 'react';
import { Button } from '../atoms';

// OAuth providers that support login flow
const OAUTH_PROVIDERS = new Set(['anthropic', 'google', 'github-copilot', 'openai']);

const DEFAULT_MODELS: Record<string, string> = {
  anthropic: 'claude-sonnet-4-20250514',
  google: 'gemini-2.0-flash',
  groq: 'llama-3.3-70b-versatile',
  ollama: 'llama3.1:8b',
  openai: 'gpt-4o',
  openrouter: 'anthropic/claude-sonnet-4-20250514',
};

interface AgentSectionProps {
  settings: {
    agent_model_provider: string | null;
    agent_model_id: string | null;
    agent_thinking_level: string | null;
    agent_base_url: string | null;
  };
  oauthLoading: boolean;
  oauthStatus: string | null;
  onOAuthStart: (provider: string) => void;
}

function AgentApiKeyField({ provider, oauthLoading, oauthStatus, onOAuthStart }: {
  provider: string;
  oauthLoading: boolean;
  oauthStatus: string | null;
  onOAuthStart: () => void;
}) {
  const [apiKey, setApiKey] = useState('');
  const [maskedKey, setMaskedKey] = useState<string | null>(null);
  const [authType, setAuthType] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [editing, setEditing] = useState(false);

  const hasOAuth = OAUTH_PROVIDERS.has(provider);

  // Load auth status on mount and when provider changes
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { agentGetAuth } = await import('../../tauri/agent');
        const auth = await agentGetAuth();
        if (!cancelled && auth[provider]) {
          setAuthType(auth[provider].type);
          setMaskedKey(auth[provider].type === 'oauth' ? 'OAuth connected' : (auth[provider].masked_key ?? '(configured)'));
          setEditing(false);
        } else if (!cancelled) {
          setAuthType(null);
          setMaskedKey(null);
          setEditing(true);
        }
      } catch {
        // ignore
      }
    })();
    return () => { cancelled = true; };
  }, [provider]);

  // Update maskedKey on OAuth success (detected via oauthLoading going false with no status)
  useEffect(() => {
    if (!oauthLoading && oauthStatus === null && authType === null) {
      // Re-check auth status after OAuth completes
      (async () => {
        try {
          const { agentGetAuth } = await import('../../tauri/agent');
          const auth = await agentGetAuth();
          if (auth[provider]) {
            setAuthType(auth[provider].type);
            setMaskedKey(auth[provider].type === 'oauth' ? 'OAuth connected' : (auth[provider].masked_key ?? '(configured)'));
            setEditing(false);
          }
        } catch {
          // ignore
        }
      })();
    }
  }, [oauthLoading, oauthStatus, provider, authType]);

  const handleSave = async () => {
    if (!apiKey.trim()) return;
    setSaving(true);
    try {
      const { agentSetApiKey } = await import('../../tauri/agent');
      await agentSetApiKey(provider, apiKey.trim());
      setAuthType('api_key');
      setMaskedKey(apiKey.length > 8 ? `${apiKey.slice(0, 4)}…${apiKey.slice(-4)}` : '****');
      setApiKey('');
      setEditing(false);
    } catch (err) {
      console.error('Failed to save API key:', err);
    } finally {
      setSaving(false);
    }
  };

  const handleRemove = async () => {
    try {
      const { agentRemoveAuth } = await import('../../tauri/agent');
      await agentRemoveAuth(provider);
      setAuthType(null);
      setMaskedKey(null);
      setEditing(true);
    } catch (err) {
      console.error('Failed to remove auth:', err);
    }
  };

  // Configured state — show current auth with change/remove
  if (!editing && maskedKey) {
    return (
      <div className="flex flex-col gap-1">
        <label className="text-tan-muted text-xs">Authentication</label>
        <div className="flex items-center gap-2">
          <span className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-tan-muted font-mono text-sm flex-1">
            {authType === 'oauth' ? '🔗 ' : '🔑 '}{maskedKey}
          </span>
          <button
            onClick={() => setEditing(true)}
            className="text-xs text-tan-muted hover:text-beige-warm transition-colors cursor-pointer"
          >
            Change
          </button>
          <button
            onClick={handleRemove}
            className="text-xs text-red-3 hover:text-red-2 transition-colors cursor-pointer"
          >
            Remove
          </button>
        </div>
      </div>
    );
  }

  // Editing state — show OAuth button + API key input
  return (
    <div className="flex flex-col gap-2">
      <label className="text-tan-muted text-xs">Authentication</label>

      {/* OAuth login */}
      {hasOAuth && (
        <div className="flex flex-col gap-1">
          {oauthLoading ? (
            <div className="flex items-center gap-2 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light">
              <span className="inline-block w-3 h-3 border-2 border-purple-1 border-t-transparent rounded-full animate-spin" />
              <span className="text-sm text-tan-muted">{oauthStatus}</span>
            </div>
          ) : (
            <Button
              text={`Sign in with ${provider.charAt(0).toUpperCase() + provider.slice(1)}`}
              size="sm"
              color="purple"
              onClick={onOAuthStart}
            />
          )}
        </div>
      )}

      {hasOAuth && (
        <div className="flex items-center gap-2">
          <div className="flex-1 h-px bg-charcoal-light" />
          <span className="text-xs text-tan-muted">or use API key</span>
          <div className="flex-1 h-px bg-charcoal-light" />
        </div>
      )}

      {/* API key input */}
      <div className="flex items-center gap-2">
        <input
          type="password"
          placeholder={`Enter ${provider} API key`}
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') handleSave(); }}
          className="flex-1 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
        />
        <Button
          text={saving ? 'Saving…' : 'Save'}
          size="sm"
          disabled={!apiKey.trim() || saving}
          onClick={handleSave}
        />
        {maskedKey && (
          <button
            onClick={() => { setEditing(false); setApiKey(''); }}
            className="text-xs text-tan-muted hover:text-beige-warm transition-colors cursor-pointer"
          >
            Cancel
          </button>
        )}
      </div>
    </div>
  );
}

export function AgentSection({ settings, oauthLoading, oauthStatus, onOAuthStart }: AgentSectionProps) {
  // Local state for optimistic UI updates — avoids waiting for IPC round-trip
  const [provider, setProvider] = useState(settings.agent_model_provider ?? 'anthropic');
  const [modelId, setModelId] = useState(settings.agent_model_id ?? '');
  const [thinkingLevel, setThinkingLevel] = useState(settings.agent_thinking_level ?? 'low');
  const [baseUrl, setBaseUrl] = useState(settings.agent_base_url ?? 'http://localhost:11434/v1');

  // Sync local state when store updates from external sources (e.g., another window)
  useEffect(() => { setProvider(settings.agent_model_provider ?? 'anthropic'); }, [settings.agent_model_provider]);
  useEffect(() => { setModelId(settings.agent_model_id ?? ''); }, [settings.agent_model_id]);
  useEffect(() => { setThinkingLevel(settings.agent_thinking_level ?? 'low'); }, [settings.agent_thinking_level]);
  useEffect(() => { setBaseUrl(settings.agent_base_url ?? 'http://localhost:11434/v1'); }, [settings.agent_base_url]);

  const save = async (updates: Record<string, string | null>) => {
    try {
      const { saveAgentSettings } = await import('../../tauri/agent');
      await saveAgentSettings(updates);
    } catch (err) {
      console.error('Failed to save agent settings:', err);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-beige-light text-lg font-semibold">AI Agent</h2>
      <p className="text-tan-muted text-xs">
        Configure the embedded AI assistant. It can build WASM components, deploy services, and manage the node.
        Requires Node.js installed.
      </p>

      {/* Provider */}
      <div className="flex flex-col gap-1">
        <label className="text-tan-muted text-xs">Provider</label>
        <select
          value={provider}
          onChange={(e) => {
            setProvider(e.target.value);
            save({ agent_model_provider: e.target.value });
          }}
          className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm outline-none"
        >
          <option value="anthropic">Anthropic</option>
          <option value="google">Google</option>
          <option value="groq">Groq</option>
          <option value="ollama">Ollama</option>
          <option value="openai">OpenAI</option>
          <option value="openrouter">OpenRouter</option>
        </select>
      </div>

      {/* Base URL (Ollama only) */}
      {provider === 'ollama' && (
        <div className="flex flex-col gap-1">
          <label className="text-tan-muted text-xs">Base URL</label>
          <input
            type="text"
            placeholder="http://localhost:11434/v1"
            value={baseUrl}
            onChange={(e) => {
              setBaseUrl(e.target.value);
              save({ agent_base_url: e.target.value || null });
            }}
            className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
          />
        </div>
      )}

      {/* Model */}
      <div className="flex flex-col gap-1">
        <label className="text-tan-muted text-xs">Model</label>
        <input
          type="text"
          placeholder={DEFAULT_MODELS[provider] ?? 'enter model id'}
          value={modelId}
          onChange={(e) => {
            setModelId(e.target.value);
            save({ agent_model_id: e.target.value || null });
          }}
          className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
        />
      </div>

      {/* Thinking level */}
      <div className="flex flex-col gap-1">
        <label className="text-tan-muted text-xs">Thinking level</label>
        <select
          value={thinkingLevel}
          onChange={(e) => {
            setThinkingLevel(e.target.value);
            save({ agent_thinking_level: e.target.value });
          }}
          className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm outline-none"
        >
          <option value="off">Off</option>
          <option value="low">Low</option>
          <option value="medium">Medium</option>
          <option value="high">High</option>
        </select>
      </div>

      {/* API Key (hidden for Ollama — no key needed) */}
      {provider !== 'ollama' && (
        <AgentApiKeyField
          provider={provider}
          oauthLoading={oauthLoading}
          oauthStatus={oauthStatus}
          onOAuthStart={() => onOAuthStart(provider)}
        />
      )}
    </div>
  );
}
