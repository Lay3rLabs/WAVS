import { clsx } from 'clsx';
import type { ReactNode } from 'react';

/* ── Tag / Badge ───────────────────────────────────────────────── */

export type TagTone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';
export type TagVariant = 'soft' | 'solid' | 'outline';

interface TagProps {
  children: ReactNode;
  tone?: TagTone;
  variant?: TagVariant;
  leading?: ReactNode;
  className?: string;
  uppercase?: boolean;
  mono?: boolean;
}

const softMap: Record<TagTone, string> = {
  neutral: 'bg-ink-surface-raised text-ink-fg-secondary border border-ink-border',
  accent:  'bg-ink-accent-tint    text-ink-accent          border border-ink-accent-edge',
  success: 'bg-ink-success-tint   text-ink-success         border border-ink-success-edge',
  warning: 'bg-ink-warning-tint   text-ink-warning         border border-ink-warning-edge',
  danger:  'bg-ink-danger-tint    text-ink-danger          border border-ink-danger-edge',
  info:    'bg-ink-info-tint      text-ink-info            border border-ink-info-edge',
};

const solidMap: Record<TagTone, string> = {
  neutral: 'bg-ink-fg-muted    text-ink-bg',
  accent:  'bg-ink-accent      text-ink-accent-fg',
  success: 'bg-ink-success     text-ink-bg',
  warning: 'bg-ink-warning     text-ink-bg',
  danger:  'bg-ink-danger      text-ink-bg',
  info:    'bg-ink-info        text-ink-bg',
};

const outlineMap: Record<TagTone, string> = {
  neutral: 'text-ink-fg-secondary border border-ink-border',
  accent:  'text-ink-accent       border border-ink-accent-edge',
  success: 'text-ink-success      border border-ink-success-edge',
  warning: 'text-ink-warning      border border-ink-warning-edge',
  danger:  'text-ink-danger       border border-ink-danger-edge',
  info:    'text-ink-info         border border-ink-info-edge',
};

export function Tag({
  children, tone = 'neutral', variant = 'soft', leading, uppercase, mono, className,
}: TagProps) {
  const variantMap = variant === 'solid' ? solidMap : variant === 'outline' ? outlineMap : softMap;
  return (
    <span
      className={clsx(
        'inline-flex items-center gap-1 px-1.5 h-5 rounded-ds-xs text-xs font-medium',
        variantMap[tone],
        uppercase && 'uppercase tracking-widest',
        mono && 'font-mono',
        className,
      )}
    >
      {leading && <span className="shrink-0">{leading}</span>}
      {children}
    </span>
  );
}

/* ── Status (dot + label) ──────────────────────────────────────── */

export type StatusTone = 'idle' | 'live' | 'pending' | 'error' | 'paused';

const statusMap: Record<StatusTone, { label: string; color: string; pulse: boolean }> = {
  live:    { label: 'Live',    color: 'bg-ink-success', pulse: true },
  pending: { label: 'Pending', color: 'bg-ink-warning', pulse: true },
  error:   { label: 'Error',   color: 'bg-ink-danger',  pulse: false },
  paused:  { label: 'Paused',  color: 'bg-ink-fg-muted', pulse: false },
  idle:    { label: 'Idle',    color: 'bg-ink-fg-faint', pulse: false },
};

interface StatusProps {
  tone: StatusTone;
  label?: string;
  className?: string;
}

export function Status({ tone, label, className }: StatusProps) {
  const s = statusMap[tone];
  return (
    <span className={clsx('inline-flex items-center gap-1.5 text-xs font-medium text-ink-fg-secondary', className)}>
      <span className="relative inline-flex h-1.5 w-1.5">
        {s.pulse && (
          <span
            aria-hidden
            className={clsx('absolute inset-0 rounded-ds-pill animate-pulse-dot opacity-60', s.color)}
          />
        )}
        <span className={clsx('relative inline-block h-1.5 w-1.5 rounded-ds-pill', s.color)} />
      </span>
      <span className="font-mono uppercase tracking-widest">{label ?? s.label}</span>
    </span>
  );
}
