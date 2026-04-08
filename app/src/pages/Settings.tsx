import { useState, useEffect } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Button, Toast } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { restart } from '../tauri';
import { SettingsSidebar, type SectionKey } from '../components/settings/SettingsSidebar';
import { WalletSection } from '../components/settings/WalletSection';
import { NodeSection } from '../components/settings/NodeSection';
import { EnvironmentSection } from '../components/settings/EnvironmentSection';
import { AgentSection } from '../components/settings/AgentSection';
import { McpSection } from '../components/settings/McpSection';
import { ResetSection } from '../components/settings/ResetSection';

export function Settings() {
  const settings = useAppStore((state) => state.settings);

  const [activeSection, setActiveSection] = useState<SectionKey>('wallet');
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const [oauthLoading, setOauthLoading] = useState(false);
  const [oauthStatus, setOauthStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  const handleRestart = async () => {
    try {
      await restart();
    } catch (err) {
      console.error('Failed to restart application:', err);
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
            <AgentSection
              settings={{
                agent_model_provider: settings.agent_model_provider,
                agent_model_id: settings.agent_model_id,
                agent_thinking_level: settings.agent_thinking_level,
                agent_base_url: settings.agent_base_url,
              }}
              oauthLoading={oauthLoading}
              oauthStatus={oauthStatus}
              onOAuthStart={handleOAuthStart}
            />
          )}
          {activeSection === 'mcp' && (
            <McpSection settings={{ mcp_auto_start: settings.mcp_auto_start, mcp_token: settings.mcp_token }} />
          )}
          {activeSection === 'reset' && (
            <ResetSection onError={setError} />
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
