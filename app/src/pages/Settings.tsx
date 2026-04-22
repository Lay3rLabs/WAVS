import { useState, useEffect, useRef } from 'react';
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
  const scrollContainerRef = useRef<HTMLDivElement>(null);

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

  // IntersectionObserver — updates sidebar highlight as user scrolls
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const sectionKeys: SectionKey[] = ['wallet', 'node', 'environment', 'agent', 'mcp', 'reset'];
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((e) => e.isIntersecting);
        if (visible.length > 0) {
          const top = visible.reduce((a, b) => (a.intersectionRatio > b.intersectionRatio ? a : b));
          const key = top.target.id.replace('section-', '') as SectionKey;
          setActiveSection(key);
        }
      },
      { root: container, threshold: 0.3 }
    );

    sectionKeys.forEach((key) => {
      const el = document.getElementById(`section-${key}`);
      if (el) observer.observe(el);
    });

    return () => observer.disconnect();
  }, []);

  const handleSidebarSelect = (key: SectionKey) => {
    document.getElementById(`section-${key}`)?.scrollIntoView({ behavior: 'smooth' });
  };

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
        <SettingsSidebar activeSection={activeSection} onSelect={handleSidebarSelect} />
        <div ref={scrollContainerRef} className="flex-1 overflow-y-auto px-6 py-4 max-h-[calc(100vh-12rem)]">
          <div id="section-wallet" className="py-8 border-b border-charcoal-light">
            <h2 className="text-lg font-semibold text-beige-light mb-4">Wallet</h2>
            <WalletSection onError={setError} />
          </div>
          <div id="section-node" className="py-8 border-b border-charcoal-light">
            <h2 className="text-lg font-semibold text-beige-light mb-4">Node</h2>
            <NodeSection
              wavsHome={settings.wavs_home}
              onUnsavedChange={setHasUnsavedChanges}
              onChanged={() => setHasUnsavedChanges(true)}
              onError={setError}
            />
          </div>
          <div id="section-environment" className="py-8 border-b border-charcoal-light">
            <h2 className="text-lg font-semibold text-beige-light mb-4">Environment</h2>
            <EnvironmentSection settings={{ saved_services: settings.saved_services, env_vars: settings.env_vars }} />
          </div>
          <div id="section-agent" className="py-8 border-b border-charcoal-light">
            <h2 className="text-lg font-semibold text-beige-light mb-4">Agent</h2>
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
          </div>
          <div id="section-mcp" className="py-8 border-b border-charcoal-light">
            <h2 className="text-lg font-semibold text-beige-light mb-4">MCP</h2>
            <McpSection settings={{ mcp_auto_start: settings.mcp_auto_start, mcp_token: settings.mcp_token }} />
          </div>
          <div id="section-reset" className="py-8">
            <h2 className="text-lg font-semibold text-beige-light mb-4">Reset</h2>
            <ResetSection onError={setError} />
          </div>

          {/* Error display */}
          {error && (
            <p className="text-red-4 text-base mt-4">{error}</p>
          )}
        </div>
      </div>
    </div>
  );
}
