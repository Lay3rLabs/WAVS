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
  return (
    <div className="flex flex-col w-[200px] shrink-0 border-r border-charcoal-light py-2">
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
    </div>
  );
}
