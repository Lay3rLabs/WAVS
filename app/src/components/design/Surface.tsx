import { clsx } from 'clsx';
import { createElement, type ReactNode } from 'react';

/* ── Surface (card) ────────────────────────────────────────────── */

interface SurfaceProps {
  children: ReactNode;
  variant?: 'flat' | 'raised' | 'sunken' | 'outline';
  className?: string;
}

export function Surface({ children, variant = 'flat', className }: SurfaceProps) {
  const v = {
    flat:    'bg-ink-surface border border-ink-border',
    raised:  'bg-ink-surface-raised border border-ink-border',
    sunken:  'bg-ink-surface-sunken border border-ink-border',
    outline: 'bg-transparent border border-ink-border',
  }[variant];

  return (
    <div className={clsx('rounded-ds-sm', v, className)}>
      {children}
    </div>
  );
}

/* ── Section / SectionHeader ───────────────────────────────────── */

interface SectionHeaderProps {
  eyebrow?: string;
  title: string;
  description?: string;
  trailing?: ReactNode;
  level?: 1 | 2 | 3;
  className?: string;
}

export function SectionHeader({
  eyebrow, title, description, trailing, level = 2, className,
}: SectionHeaderProps) {
  const sizeClass = level === 1 ? 'text-2xl' : level === 2 ? 'text-xl' : 'text-lg';
  const heading = createElement(
    `h${level}`,
    { className: clsx('text-ink-fg font-medium', sizeClass) },
    title,
  );
  return (
    <div className={clsx('flex items-start justify-between gap-6', className)}>
      <div className="flex flex-col gap-1.5">
        {eyebrow && (
          <div className="font-mono text-xs text-ink-fg-muted uppercase tracking-widest">{eyebrow}</div>
        )}
        {heading}
        {description && (
          <p className="text-sm text-ink-fg-secondary max-w-prose">{description}</p>
        )}
      </div>
      {trailing && <div className="shrink-0">{trailing}</div>}
    </div>
  );
}

/* ── Divider ───────────────────────────────────────────────────── */

interface DividerProps {
  className?: string;
  label?: string;
}

export function Divider({ className, label }: DividerProps) {
  if (label) {
    return (
      <div className={clsx('flex items-center gap-3', className)}>
        <span className="flex-1 h-px bg-ink-border" />
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">{label}</span>
        <span className="flex-1 h-px bg-ink-border" />
      </div>
    );
  }
  return <hr className={clsx('border-0 h-px bg-ink-border', className)} />;
}

/* ── Kbd (keyboard shortcut) ───────────────────────────────────── */

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="inline-flex items-center justify-center min-w-[1.5rem] h-5 px-1.5 font-mono text-xs text-ink-fg-secondary bg-ink-surface-raised border border-ink-border rounded-ds-xs">
      {children}
    </kbd>
  );
}
