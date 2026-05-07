import { clsx } from 'clsx';
import type { ReactNode } from 'react';

export interface TabItem {
  key: string;
  label: string;
  badge?: ReactNode;
  disabled?: boolean;
}

interface TabsProps {
  items: TabItem[];
  active: string;
  onChange: (key: string) => void;
  variant?: 'underline' | 'segmented';
  className?: string;
}

export function Tabs({ items, active, onChange, variant = 'underline', className }: TabsProps) {
  if (variant === 'segmented') {
    return (
      <div className={clsx('inline-flex p-0.5 rounded-ds-xs border border-ink-border bg-ink-surface-sunken', className)}>
        {items.map((item) => {
          const isActive = item.key === active;
          return (
            <button
              key={item.key}
              type="button"
              disabled={item.disabled}
              onClick={() => onChange(item.key)}
              className={clsx(
                'px-3 h-7 text-xs font-medium rounded-ds-xs transition-colors duration-ds-fast ease-ds cursor-pointer',
                isActive
                  ? 'bg-ink-surface-raised text-ink-fg'
                  : 'text-ink-fg-muted hover:text-ink-fg',
                item.disabled && 'opacity-40 cursor-not-allowed',
              )}
            >
              {item.label}
              {item.badge && <span className="ml-1.5">{item.badge}</span>}
            </button>
          );
        })}
      </div>
    );
  }

  return (
    <div role="tablist" className={clsx('flex items-center gap-1 border-b border-ink-border', className)}>
      {items.map((item) => {
        const isActive = item.key === active;
        return (
          <button
            key={item.key}
            type="button"
            role="tab"
            aria-selected={isActive}
            disabled={item.disabled}
            onClick={() => onChange(item.key)}
            className={clsx(
              'relative inline-flex items-center gap-2 h-9 px-3 text-sm font-medium cursor-pointer',
              'transition-colors duration-ds-fast ease-ds',
              isActive
                ? 'text-ink-fg'
                : 'text-ink-fg-muted hover:text-ink-fg-secondary',
              item.disabled && 'opacity-40 cursor-not-allowed',
            )}
          >
            {item.label}
            {item.badge && <span>{item.badge}</span>}
            {isActive && (
              <span aria-hidden className="absolute left-0 right-0 -bottom-px h-px bg-ink-accent" />
            )}
          </button>
        );
      })}
    </div>
  );
}
