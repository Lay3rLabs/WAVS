import { clsx } from 'clsx';
import { useEffect, useMemo, useRef, useState, type ReactNode, type KeyboardEvent } from 'react';

export interface PaletteItem {
  key: string;
  label: string;
  description?: string;
  icon?: ReactNode;
  trailing?: ReactNode;
  keywords?: string[];
  onSelect?: () => void;
}

export interface PaletteGroup {
  label: string;
  items: PaletteItem[];
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  groups: PaletteGroup[];
  placeholder?: string;
  emptyLabel?: string;
}

export function CommandPalette({
  open,
  onClose,
  groups,
  placeholder = 'Type a command or search…',
  emptyLabel = 'No results',
}: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [activeIdx, setActiveIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Filter results
  const filtered = useMemo(() => {
    if (!query) return groups;
    const q = query.toLowerCase();
    return groups
      .map((g) => ({
        ...g,
        items: g.items.filter((it) => {
          const hay = [it.label, it.description, ...(it.keywords ?? [])]
            .filter(Boolean)
            .join(' ')
            .toLowerCase();
          return hay.includes(q);
        }),
      }))
      .filter((g) => g.items.length > 0);
  }, [groups, query]);

  // Flat list for keyboard navigation
  const flat = useMemo(
    () => filtered.flatMap((g) => g.items.map((it) => ({ ...it, group: g.label }))),
    [filtered]
  );

  useEffect(() => {
    if (open) {
      setQuery('');
      setActiveIdx(0);
      const id = window.setTimeout(() => inputRef.current?.focus(), 30);
      return () => window.clearTimeout(id);
    }
  }, [open]);

  useEffect(() => { setActiveIdx(0); }, [query]);

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIdx((i) => Math.min(flat.length - 1, i + 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIdx((i) => Math.max(0, i - 1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      flat[activeIdx]?.onSelect?.();
      onClose();
    }
  };

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      className="fixed inset-0 z-50 flex items-start justify-center pt-[10vh] px-4"
    >
      {/* Backdrop */}
      <button
        type="button"
        aria-label="Close command palette"
        onClick={onClose}
        className="absolute inset-0 bg-ink-canvas/70 backdrop-blur-[2px] cursor-default"
      />

      {/* Panel */}
      <div className="relative w-full max-w-[640px] bg-ink-surface-overlay border border-ink-border-strong rounded-ds-md overflow-hidden shadow-[0_0_0_1px_var(--color-canvas)]">
        {/* Input row */}
        <div className="flex items-center gap-2 h-11 px-4 border-b border-ink-border">
          <svg width="13" height="13" viewBox="0 0 12 12" fill="none" className="text-ink-fg-muted shrink-0">
            <circle cx="5" cy="5" r="3.2" stroke="currentColor" strokeWidth="1.2" />
            <path d="M7.5 7.5L10 10" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          </svg>
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={placeholder}
            spellCheck={false}
            autoComplete="off"
            className="flex-1 bg-transparent outline-none text-sm text-ink-fg placeholder:text-ink-fg-faint"
          />
          <kbd className="font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted bg-ink-surface-raised border border-ink-border rounded-ds-xs px-1.5 h-5 inline-flex items-center">esc</kbd>
        </div>

        {/* Results */}
        <div ref={listRef} className="max-h-[420px] overflow-y-auto py-1">
          {flat.length === 0 ? (
            <div className="px-4 py-12 text-center">
              <p className="text-sm text-ink-fg-muted">{emptyLabel}</p>
              {query && <p className="font-mono text-xs text-ink-fg-faint mt-1">{`for "${query}"`}</p>}
            </div>
          ) : (
            filtered.map((g) => {
              const groupStartIdx = flat.findIndex((f) => f.group === g.label);
              return (
                <div key={g.label} className="flex flex-col py-1">
                  <div className="px-4 pt-1.5 pb-1 font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted">
                    {g.label}
                  </div>
                  {g.items.map((it, i) => {
                    const flatIdx = groupStartIdx + i;
                    const active = flatIdx === activeIdx;
                    return (
                      <button
                        key={it.key}
                        type="button"
                        onMouseEnter={() => setActiveIdx(flatIdx)}
                        onClick={() => { it.onSelect?.(); onClose(); }}
                        className={clsx(
                          'flex items-center gap-3 px-4 h-9 text-left cursor-pointer transition-colors duration-ds-fast',
                          active ? 'bg-ink-surface-raised' : 'hover:bg-ink-surface-raised/50',
                        )}
                      >
                        {it.icon && <span className={clsx('shrink-0', active ? 'text-ink-accent' : 'text-ink-fg-muted')}>{it.icon}</span>}
                        <span className="flex-1 min-w-0 flex items-baseline gap-2">
                          <span className={clsx('text-sm truncate', active ? 'text-ink-fg' : 'text-ink-fg-secondary')}>{it.label}</span>
                          {it.description && (
                            <span className="text-xs text-ink-fg-muted truncate">{it.description}</span>
                          )}
                        </span>
                        {it.trailing && <span className="shrink-0">{it.trailing}</span>}
                        {active && (
                          <kbd className="font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted bg-ink-surface border border-ink-border rounded-ds-xs px-1.5 h-5 inline-flex items-center shrink-0">↩</kbd>
                        )}
                      </button>
                    );
                  })}
                </div>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-4 h-9 border-t border-ink-border bg-ink-surface-sunken">
          <span className="font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted">
            {flat.length > 0 ? `${flat.length} result${flat.length === 1 ? '' : 's'}` : 'no results'}
          </span>
          <div className="flex items-center gap-3 font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted">
            <span className="flex items-center gap-1.5">
              <kbd className="bg-ink-surface-raised border border-ink-border rounded-ds-xs px-1.5 h-4 inline-flex items-center">↑</kbd>
              <kbd className="bg-ink-surface-raised border border-ink-border rounded-ds-xs px-1.5 h-4 inline-flex items-center">↓</kbd>
              navigate
            </span>
            <span className="flex items-center gap-1.5">
              <kbd className="bg-ink-surface-raised border border-ink-border rounded-ds-xs px-1.5 h-4 inline-flex items-center">↩</kbd>
              select
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
