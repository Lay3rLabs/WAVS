import { useNavigate } from 'react-router-dom';

export type SectionKey = 'wallet' | 'node' | 'environment' | 'agent' | 'mcp' | 'reset';

const SIDEBAR_ITEMS: { key: SectionKey; label: string }[] = [
  { key: 'wallet', label: 'Wallet' },
  { key: 'node', label: 'Node' },
  { key: 'environment', label: 'Environment' },
  { key: 'agent', label: 'Agent' },
  { key: 'mcp', label: 'MCP' },
  { key: 'reset', label: 'Reset' },
];

interface SettingsSidebarProps {
  activeSection: SectionKey;
  onSelect: (key: SectionKey) => void;
}

export function SettingsSidebar({ activeSection, onSelect }: SettingsSidebarProps) {
  const navigate = useNavigate();
  return (
    <div className="flex flex-col w-[200px] shrink-0 border-r border-charcoal-light py-2 sticky top-0 self-start">
      {SIDEBAR_ITEMS.map((item) => {
        const isActive = item.key === activeSection;
        return (
          <button
            key={item.key}
            onClick={() => onSelect(item.key)}
            className={`w-full text-left px-3 py-2 text-sm transition-colors cursor-pointer ${
              isActive
                ? 'text-beige-light font-semibold border-l-2 border-purple-2 bg-charcoal-medium'
                : 'text-tan-muted font-normal hover:text-beige-warm hover:bg-charcoal-medium border-l-2 border-transparent'
            }`}
          >
            {item.label}
          </button>
        );
      })}

      {/* External link out to the design system */}
      <div className="mt-2 pt-2 border-t border-charcoal-light">
        <button
          onClick={() => navigate('/design')}
          className="w-full text-left px-3 py-2 text-sm font-normal cursor-pointer transition-colors border-l-2 border-transparent text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium flex items-center justify-between gap-2"
        >
          <span>Design system</span>
          <svg width="10" height="10" viewBox="0 0 12 12" fill="none" className="opacity-60">
            <path d="M3 9L9 3M9 3H4M9 3V8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
      </div>
    </div>
  );
}
