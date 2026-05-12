import { useState } from 'react';
import { Button } from '../../components/atoms';
import { useAppStore } from '../../stores/appStore';
import { useWalletStore } from '../../stores/walletStore';
import { restart } from '../../tauri';
import { WalletSection } from './WalletSection';
import { WavsHomeSection } from './WavsHomeSection';
import { TomlEditorSection } from './TomlEditorSection';
import { EnvVariablesSection } from './EnvVariablesSection';
import { McpServerSection } from './McpServerSection';
import { ResetAppSection } from './ResetAppSection';

const SECTIONS = [
  { id: 'wallet', label: 'Wallet' },
  { id: 'wavs-home', label: 'WAVS Home' },
  { id: 'toml-editor', label: 'Configuration' },
  { id: 'env-vars', label: 'Environment Variables' },
  { id: 'mcp-server', label: 'MCP Server' },
  { id: 'reset', label: 'Reset App State' },
] as const;

export function Settings() {
  const settings = useAppStore((state) => state.settings);
  const walletError = useWalletStore((state) => state.error);
  const [changed, setChanged] = useState(false);
  const [activeSection, setActiveSection] = useState<string>('wallet');

  const scrollToSection = (id: string) => {
    document.getElementById(id)?.scrollIntoView({ behavior: 'smooth' });
    setActiveSection(id);
  };

  const handleRestart = async () => {
    try {
      await restart();
    } catch (err) {
      console.error('Failed to restart application:', err);
    }
  };

  return (
    <div className="flex gap-6 max-h-[calc(100vh-12rem)]">
      {/* Sticky sidebar navigation */}
      <nav className="w-48 shrink-0 sticky top-0 self-start flex flex-col gap-1">
        {SECTIONS.map(({ id, label }) => (
          <button
            key={id}
            onClick={() => scrollToSection(id)}
            className={`text-left px-3 py-2 rounded text-sm transition-colors ${
              activeSection === id
                ? 'text-beige-light bg-charcoal-medium'
                : 'text-tan-muted hover:text-beige-warm'
            }`}
          >
            {label}
          </button>
        ))}
      </nav>

      {/* Scrollable content area */}
      <div className="flex-1 overflow-y-auto pr-2 flex flex-col gap-6">
        {/* Restart banner */}
        {changed && (
          <div className="flex gap-4 mb-4 items-center">
            <div className="flex-1 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
              <p className="text-lg text-beige-light">
                Restart for changes to take effect.
              </p>
            </div>
            <Button
              text="Restart Application"
              color="red"
              onClick={handleRestart}
            />
          </div>
        )}

        <WalletSection />
        <WavsHomeSection onChanged={() => setChanged(true)} />
        {settings.wavs_home && (
          <TomlEditorSection onChanged={() => setChanged(true)} />
        )}
        <EnvVariablesSection />
        <McpServerSection />
        <ResetAppSection />

        {/* Global wallet error display */}
        {walletError && (
          <p className="text-red-4 text-base">{walletError}</p>
        )}
      </div>
    </div>
  );
}
