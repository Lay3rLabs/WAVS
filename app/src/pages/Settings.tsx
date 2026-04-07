import { useState, useEffect } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Button, Toast } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { restart, startMcpServer, stopMcpServer, getMcpStatus, getMcpBinaryPath, getWavsUrl, saveMcpSettings, clearPersistedServices, registerClaudeMcp, pickFolder } from '../tauri';
import { usePOAStore } from '../stores/poaStore';
import { errorMessage } from '../utils/error';
import type { McpStatus } from '../types';
import { SettingsSidebar, type SectionKey } from '../components/settings/SettingsSidebar';
import { WalletSection } from '../components/settings/WalletSection';
import { NodeSection } from '../components/settings/NodeSection';
import { EnvironmentSection } from '../components/settings/EnvironmentSection';

// OAuth providers that support login flow
const OAUTH_PROVIDERS = new Set(['anthropic', 'google', 'github-copilot', 'openai']);

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
        const { agentGetAuth } = await import('../tauri/agent');
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
          const { agentGetAuth } = await import('../tauri/agent');
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
      const { agentSetApiKey } = await import('../tauri/agent');
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
      const { agentRemoveAuth } = await import('../tauri/agent');
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

export function Settings() {
  const settings = useAppStore((state) => state.settings);

  const [activeSection, setActiveSection] = useState<SectionKey>('wallet');
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [oauthLoading, setOauthLoading] = useState(false);
  const [oauthStatus, setOauthStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // MCP server state
  const [mcpStatus, setMcpStatus] = useState<McpStatus | null>(null);
  const [mcpBinaryPath, setMcpBinaryPath] = useState<string | null>(null);
  const [wavsUrl, setWavsUrl] = useState('http://localhost:8000');
  const [mcpAutoStart, setMcpAutoStart] = useState(settings.mcp_auto_start ?? false);
  const [mcpToken, setMcpToken] = useState(settings.mcp_token ?? '');
  const [mcpLoading, setMcpLoading] = useState(false);
  const [mcpError, setMcpError] = useState<string | null>(null);
  const [claudeProjectPath, setClaudeProjectPath] = useState('');
  const [claudeRegisterResult, setClaudeRegisterResult] = useState<string | null>(null);
  const [claudeRegisterLoading, setClaudeRegisterLoading] = useState(false);
  const [claudeRegisterError, setClaudeRegisterError] = useState<string | null>(null);
  const [showClearServicesConfirm, setShowClearServicesConfirm] = useState(false);

  // OAuth listener in parent — survives section navigation
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    listen<{ type: string; url?: string; message?: string; provider?: string }>(
      'agent:oauth',
      (event) => {
        const data = event.payload;
        switch (data.type) {
          case 'open_url':
            setOauthStatus('Waiting for browser authorization…');
            break;
          case 'progress':
            setOauthStatus(data.message ?? 'Working…');
            break;
          case 'success':
            setOauthStatus(null);
            setOauthLoading(false);
            break;
          case 'error':
            setOauthStatus(null);
            setOauthLoading(false);
            Toast.error(data.message ?? 'OAuth login failed');
            break;
        }
      }
    ).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  // Poll MCP status every 3 seconds; also resolve the binary path once
  useEffect(() => {
    getMcpBinaryPath().then(setMcpBinaryPath).catch(() => {});
    getWavsUrl().then(setWavsUrl).catch(() => {});

    let cancelled = false;
    const poll = async () => {
      try {
        const status = await getMcpStatus();
        if (!cancelled) setMcpStatus(status);
      } catch {
        // not fatal
      }
    };
    poll();
    const id = setInterval(poll, 3000);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  const handleOAuthStart = async (provider: string) => {
    setOauthLoading(true);
    setOauthStatus('Starting login…');
    try {
      const { agentOAuthLogin } = await import('../tauri/agent');
      await agentOAuthLogin(provider);
    } catch (err) {
      setOauthLoading(false);
      setOauthStatus(null);
      console.error('OAuth login failed:', err);
    }
  };

  const handleMcpToggle = async () => {
    setMcpLoading(true);
    setMcpError(null);
    try {
      if (mcpStatus?.running) {
        await stopMcpServer();
      } else {
        await startMcpServer();
      }
      setMcpStatus(await getMcpStatus());
    } catch (e) {
      setMcpError(errorMessage(e));
    } finally {
      setMcpLoading(false);
    }
  };

  const handleMcpSaveSettings = async () => {
    setMcpError(null);
    try {
      await saveMcpSettings(mcpAutoStart, mcpToken.trim() || null);
    } catch (e) {
      setMcpError(errorMessage(e));
    }
  };

  const handleRegisterClaude = async () => {
    setClaudeRegisterLoading(true);
    setClaudeRegisterError(null);
    setClaudeRegisterResult(null);
    try {
      const result = await registerClaudeMcp(claudeProjectPath.trim());
      setClaudeRegisterResult(result);
    } catch (e) {
      setClaudeRegisterError(errorMessage(e));
    } finally {
      setClaudeRegisterLoading(false);
    }
  };

  const handleRestart = async () => {
    try {
      await restart();
    } catch (err) {
      console.error('Failed to restart application:', err);
    }
  };

  const handleClearServices = async () => {
    setError(null);
    try {
      await clearPersistedServices();
      usePOAStore.getState().clearRegistries();
      setShowClearServicesConfirm(false);
    } catch {
      setError('Failed to clear app state. Please try again.');
    }
  };

  return (
    <div className="flex flex-col gap-0">
      {/* Restart banner - always visible above sidebar+content split */}
      {hasUnsavedChanges && (
        <div className="flex gap-4 mb-4 items-center p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
          <p className="text-lg text-beige-light flex-1">Restart for changes to take effect.</p>
          <Button text="Restart Application" color="red" onClick={handleRestart} />
        </div>
      )}

      <div className="flex flex-1 gap-0">
        <SettingsSidebar activeSection={activeSection} onSelect={setActiveSection} />
        <div className="flex-1 overflow-y-auto px-6 py-4 max-h-[calc(100vh-12rem)]">
          {activeSection === 'wallet' && (
            <WalletSection onError={setError} />
          )}
          {activeSection === 'node' && (
            <NodeSection
              wavsHome={settings.wavs_home}
              onUnsavedChange={setHasUnsavedChanges}
              onChanged={() => setHasUnsavedChanges(true)}
              onError={setError}
            />
          )}
          {activeSection === 'environment' && (
            <EnvironmentSection settings={{ saved_services: settings.saved_services, env_vars: settings.env_vars }} />
          )}
          {activeSection === 'agent' && (
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
                  value={settings.agent_model_provider ?? 'anthropic'}
                  onChange={async (e) => {
                    try {
                      const { saveAgentSettings } = await import('../tauri/agent');
                      await saveAgentSettings({ agent_model_provider: e.target.value });
                    } catch (err) {
                      console.error('Failed to save agent provider:', err);
                    }
                  }}
                  className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm outline-none"
                >
                  <option value="anthropic">Anthropic</option>
                  <option value="openai">OpenAI</option>
                  <option value="google">Google</option>
                </select>
              </div>

              {/* Model */}
              <div className="flex flex-col gap-1">
                <label className="text-tan-muted text-xs">Model</label>
                <input
                  type="text"
                  placeholder="claude-sonnet-4-20250514"
                  value={settings.agent_model_id ?? ''}
                  onChange={async (e) => {
                    try {
                      const { saveAgentSettings } = await import('../tauri/agent');
                      await saveAgentSettings({ agent_model_id: e.target.value || null });
                    } catch (err) {
                      console.error('Failed to save agent model:', err);
                    }
                  }}
                  className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
                />
              </div>

              {/* Thinking level */}
              <div className="flex flex-col gap-1">
                <label className="text-tan-muted text-xs">Thinking level</label>
                <select
                  value={settings.agent_thinking_level ?? 'low'}
                  onChange={async (e) => {
                    try {
                      const { saveAgentSettings } = await import('../tauri/agent');
                      await saveAgentSettings({ agent_thinking_level: e.target.value });
                    } catch (err) {
                      console.error('Failed to save agent thinking level:', err);
                    }
                  }}
                  className="px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm outline-none"
                >
                  <option value="off">Off</option>
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                </select>
              </div>

              {/* API Key */}
              <AgentApiKeyField
                provider={settings.agent_model_provider ?? 'anthropic'}
                oauthLoading={oauthLoading}
                oauthStatus={oauthStatus}
                onOAuthStart={() => handleOAuthStart(settings.agent_model_provider ?? 'anthropic')}
              />
            </div>
          )}
          {activeSection === 'mcp' && (
            <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <h2 className="text-beige-light text-lg font-semibold">MCP Server</h2>
                  {mcpStatus && (
                    <span className={`text-xs font-mono px-2 py-0.5 rounded ${
                      mcpStatus.running
                        ? 'bg-charcoal-dark text-green-4'
                        : 'bg-charcoal-dark text-tan-muted'
                    }`}>
                      {mcpStatus.running ? `Running (pid ${mcpStatus.pid})` : 'Stopped'}
                    </span>
                  )}
                </div>
                <Button
                  text={mcpLoading ? '...' : mcpStatus?.running ? 'Stop' : 'Start'}
                  color={mcpStatus?.running ? 'red' : undefined}
                  variant="outline"
                  onClick={handleMcpToggle}
                  disabled={mcpLoading}
                />
              </div>

              <p className="text-tan-muted text-xs">
                Exposes WAVS operations to AI assistants (Claude Desktop, Cursor, VS Code) via the Model Context Protocol.
              </p>

              {/* Auto-start toggle */}
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={mcpAutoStart}
                  onChange={(e) => setMcpAutoStart(e.target.checked)}
                  className="w-4 h-4 accent-green-4"
                />
                <span className="text-beige-warm text-sm">Auto-start when WAVS node starts</span>
              </label>

              {/* Bearer token */}
              <div className="flex flex-col gap-1">
                <label className="text-tan-muted text-xs">Bearer token (for write operations)</label>
                <div className="flex gap-2">
                  <input
                    type="password"
                    placeholder="Optional — leave blank for read-only access"
                    value={mcpToken}
                    onChange={(e) => setMcpToken(e.target.value)}
                    className="flex-1 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
                  />
                  <Button
                    text="Generate"
                    variant="outline"
                    onClick={() => {
                      const bytes = new Uint8Array(24);
                      crypto.getRandomValues(bytes);
                      setMcpToken(btoa(String.fromCharCode(...bytes)).replace(/[+/=]/g, (c) => ({ '+': '-', '/': '_', '=': '' }[c] ?? c)));
                    }}
                  />
                </div>
              </div>

              <Button
                text="Save MCP Settings"
                variant="outline"
                onClick={handleMcpSaveSettings}
              />

              {/* Config snippet */}
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs">Claude Desktop / Cursor config snippet:</span>
                <pre className="text-xs font-mono text-beige-warm bg-charcoal-darkest rounded p-3 overflow-x-auto whitespace-pre-wrap">{
`{
  "mcpServers": {
    "wavs": {
      "command": "${mcpBinaryPath ?? '/path/to/wavs-mcp'}",
      "args": ["--wavs-url", "${wavsUrl}"${mcpToken.trim() ? `,\n               "--token", "${mcpToken.trim()}"` : ''}]
    }
  }
}`
                }</pre>
                {!mcpBinaryPath && (
                  <p className="text-tan-muted text-xs mt-1">
                    Binary not found. Build it with: <span className="font-mono">cargo build --release -p wavs-mcp</span>
                  </p>
                )}
              </div>

              {/* Register with Claude Code */}
              <div className="flex flex-col gap-2">
                <label className="text-tan-muted text-xs font-medium">Register with Claude Code</label>
                <p className="text-tan-muted text-xs">
                  Add wavs-mcp to a Claude Code project so MCP tools are available there.
                </p>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={claudeProjectPath}
                    onChange={(e) => setClaudeProjectPath(e.target.value)}
                    placeholder="/path/to/your-project"
                    className="flex-1 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
                  />
                  <Button
                    text="Browse..."
                    variant="outline"
                    onClick={async () => {
                      const path = await pickFolder();
                      if (path) setClaudeProjectPath(path);
                    }}
                  />
                  <Button
                    text={claudeRegisterLoading ? '...' : 'Register'}
                    variant="outline"
                    onClick={handleRegisterClaude}
                    disabled={claudeRegisterLoading || !mcpStatus?.running || !claudeProjectPath.trim()}
                  />
                </div>
                {claudeRegisterResult && (
                  <p className="text-green-4 text-xs">
                    Registered for {claudeRegisterResult}. Restart Claude Code to pick up the change.
                  </p>
                )}
                {claudeRegisterError && (
                  <p className="text-red-4 text-xs">{claudeRegisterError}</p>
                )}
              </div>

              {mcpError && <p className="text-red-4 text-sm">{mcpError}</p>}
            </div>
          )}
          {activeSection === 'reset' && (
            <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
              <h2 className="text-beige-light text-lg font-semibold">Reset App State</h2>
              <p className="text-tan-muted text-sm">
                Remove all registered services and saved registries from the app. Useful when restarting a local chain (e.g. Anvil) where previous contract addresses no longer exist.
              </p>

              {!showClearServicesConfirm && (
                <Button
                  text="Clear All Services & Registries"
                  color="red"
                  variant="outline"
                  onClick={() => setShowClearServicesConfirm(true)}
                />
              )}

              {showClearServicesConfirm && (
                <div className="flex flex-col gap-3 p-3 rounded bg-charcoal-darkest border border-red-2">
                  <p className="text-sm text-red-4">
                    This will stop all running services and clear all saved registries. They can be re-added from the Services page.
                  </p>
                  <div className="flex gap-3">
                    <Button
                      text="Keep Services"
                      variant="outline"
                      onClick={() => setShowClearServicesConfirm(false)}
                    />
                    <Button
                      text="Confirm Clear"
                      color="red"
                      onClick={handleClearServices}
                    />
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Error display */}
          {error && (
            <p className="text-red-4 text-base mt-4">{error}</p>
          )}
        </div>
      </div>
    </div>
  );
}
