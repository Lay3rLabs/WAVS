import { clsx } from 'clsx';
import { useState, type ReactNode } from 'react';

/* ── AppBar (top horizontal nav) ───────────────────────────────── */

export interface AppBarItem {
  key: string;
  label: string;
  icon?: ReactNode;
  active?: boolean;
  badge?: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
}

interface AppBarProps {
  brand?: ReactNode;
  items?: AppBarItem[];
  actions?: ReactNode;
  className?: string;
  sticky?: boolean;
  /** Force compact (icon-only) layout — otherwise auto-collapses below `md`. */
  compact?: boolean;
}

export function AppBar({ brand, items = [], actions, className, sticky, compact }: AppBarProps) {
  const [mobileOpen, setMobileOpen] = useState(false);
  return (
    <header
      className={clsx(
        'flex items-center justify-between gap-4 h-12 px-4 border-b border-ink-border bg-ink-surface',
        sticky && 'sticky top-0 z-20',
        className,
      )}
    >
      <div className="flex items-center gap-4 min-w-0">
        {brand && <div className="shrink-0 flex items-center">{brand}</div>}

        {/* Desktop nav */}
        <nav className={clsx('items-center gap-px', compact ? 'flex' : 'hidden md:flex')}>
          {items.map((item) => (
            <AppBarLink key={item.key} item={item} compact={compact} />
          ))}
        </nav>
      </div>

      <div className="flex items-center gap-2">
        {actions}
        {!compact && (
          <button
            type="button"
            onClick={() => setMobileOpen((v) => !v)}
            aria-label="Toggle navigation"
            aria-expanded={mobileOpen}
            className="md:hidden inline-flex h-8 w-8 items-center justify-center rounded-ds-xs text-ink-fg-secondary hover:bg-ink-surface-raised hover:text-ink-fg transition-colors duration-ds-fast cursor-pointer"
          >
            {mobileOpen ? (
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M3 3l8 8M11 3l-8 8" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M2 4h10M2 7h10M2 10h10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              </svg>
            )}
          </button>
        )}
      </div>

      {/* Mobile dropdown */}
      {mobileOpen && !compact && (
        <div className="md:hidden absolute top-12 left-0 right-0 bg-ink-surface border-b border-ink-border z-30 flex flex-col py-1">
          {items.map((item) => (
            <button
              key={item.key}
              type="button"
              disabled={item.disabled}
              onClick={() => { item.onClick?.(); setMobileOpen(false); }}
              className={clsx(
                'flex items-center gap-3 px-4 h-10 text-sm text-left transition-colors duration-ds-fast',
                item.active
                  ? 'text-ink-accent bg-ink-accent-tint'
                  : 'text-ink-fg-secondary hover:bg-ink-surface-raised hover:text-ink-fg',
                item.disabled && 'opacity-40 cursor-not-allowed',
              )}
            >
              {item.icon && <span className="shrink-0 w-4 flex items-center justify-center">{item.icon}</span>}
              <span className="flex-1">{item.label}</span>
              {item.badge}
            </button>
          ))}
        </div>
      )}
    </header>
  );
}

function AppBarLink({ item, compact }: { item: AppBarItem; compact?: boolean }) {
  return (
    <button
      type="button"
      disabled={item.disabled}
      onClick={item.onClick}
      title={compact ? item.label : undefined}
      className={clsx(
        'relative inline-flex items-center gap-2 h-8 px-3 rounded-ds-xs text-sm font-medium cursor-pointer',
        'transition-colors duration-ds-fast',
        item.active
          ? 'text-ink-fg bg-ink-surface-raised'
          : 'text-ink-fg-muted hover:text-ink-fg hover:bg-ink-surface-raised',
        item.disabled && 'opacity-40 cursor-not-allowed',
      )}
    >
      {item.icon && <span className="shrink-0">{item.icon}</span>}
      {!compact && <span>{item.label}</span>}
      {item.badge && !compact && <span>{item.badge}</span>}
    </button>
  );
}

/* ── SideNav (vertical) ────────────────────────────────────────── */

export interface SideNavItem {
  key: string;
  label: string;
  icon?: ReactNode;
  active?: boolean;
  badge?: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
}

export interface SideNavGroup {
  label?: string;
  items: SideNavItem[];
}

interface SideNavProps {
  brand?: ReactNode;
  groups: SideNavGroup[];
  footer?: ReactNode;
  collapsed?: boolean;
  onToggleCollapsed?: () => void;
  className?: string;
}

export function SideNav({
  brand, groups, footer, collapsed, onToggleCollapsed, className,
}: SideNavProps) {
  return (
    <aside
      className={clsx(
        'flex flex-col border-r border-ink-border bg-ink-surface',
        collapsed ? 'w-14' : 'w-56',
        'transition-[width] duration-ds-base ease-ds',
        className,
      )}
    >
      {brand && (
        <div className={clsx('flex items-center h-12 border-b border-ink-border', collapsed ? 'justify-center px-0' : 'px-4')}>
          {brand}
        </div>
      )}
      <div className="flex-1 overflow-y-auto py-2">
        {groups.map((g, gi) => (
          <div key={gi} className={clsx('flex flex-col', gi > 0 && 'mt-3')}>
            {!collapsed && g.label && (
              <div className="px-4 pt-2 pb-1 font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted">
                {g.label}
              </div>
            )}
            {g.items.map((item) => (
              <SideNavRow key={item.key} item={item} collapsed={collapsed} />
            ))}
          </div>
        ))}
      </div>
      {(footer || onToggleCollapsed) && (
        <div className={clsx('border-t border-ink-border', collapsed ? 'px-0' : 'px-2', 'py-2 flex items-center', collapsed ? 'justify-center' : 'justify-between gap-2')}>
          {!collapsed && footer}
          {onToggleCollapsed && (
            <button
              type="button"
              onClick={onToggleCollapsed}
              aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
              className="inline-flex h-7 w-7 items-center justify-center rounded-ds-xs text-ink-fg-muted hover:bg-ink-surface-raised hover:text-ink-fg cursor-pointer transition-colors duration-ds-fast"
            >
              <svg width="11" height="11" viewBox="0 0 12 12" fill="none" className={clsx('transition-transform duration-ds-base', collapsed && 'rotate-180')}>
                <path d="M8 3l-3 3 3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          )}
        </div>
      )}
    </aside>
  );
}

function SideNavRow({ item, collapsed }: { item: SideNavItem; collapsed?: boolean }) {
  return (
    <button
      type="button"
      disabled={item.disabled}
      onClick={item.onClick}
      title={collapsed ? item.label : undefined}
      className={clsx(
        'relative flex items-center gap-3 h-8 mx-2 px-2 rounded-ds-xs text-sm cursor-pointer text-left',
        'transition-colors duration-ds-fast',
        item.active
          ? 'text-ink-fg bg-ink-surface-raised'
          : 'text-ink-fg-muted hover:text-ink-fg hover:bg-ink-surface-raised',
        item.disabled && 'opacity-40 cursor-not-allowed',
        collapsed && 'justify-center px-0',
      )}
    >
      {item.active && !collapsed && (
        <span aria-hidden className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-ink-accent rounded-ds-pill" />
      )}
      {item.icon && <span className="shrink-0 w-4 flex items-center justify-center">{item.icon}</span>}
      {!collapsed && (
        <>
          <span className="flex-1 truncate">{item.label}</span>
          {item.badge}
        </>
      )}
    </button>
  );
}

/* ── Breadcrumbs ───────────────────────────────────────────────── */

export interface CrumbItem {
  label: ReactNode;
  onClick?: () => void;
  current?: boolean;
}

interface BreadcrumbsProps {
  items: CrumbItem[];
  /** Collapse middle items to '…' when count exceeds this. Default 4. */
  maxItems?: number;
  className?: string;
  separator?: ReactNode;
}

export function Breadcrumbs({ items, maxItems = 4, className, separator }: BreadcrumbsProps) {
  const sep = separator ?? (
    <span aria-hidden className="font-mono text-ink-fg-faint">/</span>
  );

  let visible: (CrumbItem | { ellipsis: true })[] = items;
  if (items.length > maxItems) {
    visible = [
      items[0],
      { ellipsis: true },
      ...items.slice(items.length - (maxItems - 2)),
    ];
  }

  return (
    <nav aria-label="Breadcrumb" className={clsx('flex items-center gap-2 min-w-0', className)}>
      {visible.map((node, i) => {
        if ('ellipsis' in node) {
          return (
            <span key={`e-${i}`} className="flex items-center gap-2">
              <span className="font-mono text-xs text-ink-fg-muted">…</span>
              {sep}
            </span>
          );
        }
        const isLast = i === visible.length - 1;
        const item = node as CrumbItem;
        return (
          <span key={i} className="flex items-center gap-2 min-w-0">
            {item.onClick && !item.current ? (
              <button
                type="button"
                onClick={item.onClick}
                className="text-xs text-ink-fg-muted hover:text-ink-fg cursor-pointer transition-colors duration-ds-fast truncate"
              >
                {item.label}
              </button>
            ) : (
              <span className={clsx('text-xs truncate', item.current ? 'text-ink-fg' : 'text-ink-fg-muted')}>
                {item.label}
              </span>
            )}
            {!isLast && sep}
          </span>
        );
      })}
    </nav>
  );
}

/* ── Pagination ────────────────────────────────────────────────── */

interface PaginationProps {
  page: number;
  pageCount: number;
  onPageChange?: (page: number) => void;
  totalItems?: number;
  pageSize?: number;
  className?: string;
}

export function Pagination({
  page, pageCount, onPageChange, totalItems, pageSize, className,
}: PaginationProps) {
  const go = (p: number) => {
    const clamped = Math.max(1, Math.min(pageCount, p));
    if (clamped !== page) onPageChange?.(clamped);
  };

  const pages = pageWindow(page, pageCount);

  return (
    <div className={clsx('flex items-center justify-between gap-3', className)}>
      <div className="font-mono text-xs text-ink-fg-muted">
        {totalItems !== undefined && pageSize !== undefined ? (
          <>
            Showing <span className="text-ink-fg-secondary">{Math.min((page - 1) * pageSize + 1, totalItems)}–{Math.min(page * pageSize, totalItems)}</span> of <span className="text-ink-fg-secondary">{totalItems.toLocaleString()}</span>
          </>
        ) : (
          <>Page <span className="text-ink-fg-secondary">{page}</span> of <span className="text-ink-fg-secondary">{pageCount}</span></>
        )}
      </div>
      <div className="flex items-center gap-1">
        <PageBtn disabled={page <= 1} onClick={() => go(page - 1)}>
          <svg width="9" height="9" viewBox="0 0 12 12" fill="none">
            <path d="M8 3l-3 3 3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </PageBtn>
        {pages.map((p, i) =>
          p === '…' ? (
            <span key={`e-${i}`} className="font-mono text-xs text-ink-fg-faint w-7 h-7 inline-flex items-center justify-center">…</span>
          ) : (
            <PageBtn key={p} active={p === page} onClick={() => go(p)}>
              {p}
            </PageBtn>
          )
        )}
        <PageBtn disabled={page >= pageCount} onClick={() => go(page + 1)}>
          <svg width="9" height="9" viewBox="0 0 12 12" fill="none">
            <path d="M4 3l3 3-3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </PageBtn>
      </div>
    </div>
  );
}

function PageBtn({
  children, active, disabled, onClick,
}: { children: ReactNode; active?: boolean; disabled?: boolean; onClick?: () => void }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={clsx(
        'inline-flex h-7 min-w-[1.75rem] px-2 items-center justify-center rounded-ds-xs cursor-pointer',
        'font-mono text-xs transition-colors duration-ds-fast',
        active
          ? 'bg-ink-accent-tint text-ink-accent border border-ink-accent-edge'
          : 'text-ink-fg-secondary border border-transparent hover:bg-ink-surface-raised hover:text-ink-fg',
        disabled && 'opacity-40 cursor-not-allowed pointer-events-none',
      )}
    >
      {children}
    </button>
  );
}

function pageWindow(page: number, total: number): (number | '…')[] {
  if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);
  const out: (number | '…')[] = [1];
  if (page > 3) out.push('…');
  for (let p = Math.max(2, page - 1); p <= Math.min(total - 1, page + 1); p++) out.push(p);
  if (page < total - 2) out.push('…');
  out.push(total);
  return out;
}
