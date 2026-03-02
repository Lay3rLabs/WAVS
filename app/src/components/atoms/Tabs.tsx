export interface Tab {
  key: string;
  label: string;
}

interface TabsProps {
  tabs: Tab[];
  activeTab: string;
  onChange: (key: string) => void;
}

export function Tabs({ tabs, activeTab, onChange }: TabsProps) {
  return (
    <div className="border-b border-charcoal-light">
      <div className="flex gap-6">
        {tabs.map((tab) => {
          const isActive = tab.key === activeTab;
          return (
            <button
              key={tab.key}
              type="button"
              onClick={() => onChange(tab.key)}
              className={`pb-2 text-sm font-medium transition-colors border-b-2 -mb-px ${
                isActive
                  ? 'border-purple-2 text-beige-light'
                  : 'border-transparent text-tan-muted hover:text-beige-warm'
              }`}
            >
              {tab.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}
