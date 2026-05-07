import { clsx } from 'clsx';
import { useState } from 'react';
import type { ReactNode } from 'react';

/* ── Address (truncated mono with copy) ────────────────────────── */

interface AddressProps {
  value: string;
  truncate?: boolean | number;
  copyable?: boolean;
  className?: string;
}

export function Address({ value, truncate = true, copyable = true, className }: AddressProps) {
  const [copied, setCopied] = useState(false);
  const visible = truncate
    ? truncateAddress(value, typeof truncate === 'number' ? truncate : 6)
    : value;

  const copy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch { /* ignore */ }
  };

  return (
    <span
      onClick={copyable ? copy : undefined}
      className={clsx(
        'inline-flex items-center gap-1.5 font-mono text-xs',
        'text-ink-fg-secondary',
        copyable && 'cursor-pointer hover:text-ink-fg group',
        className,
      )}
      title={value}
    >
      <span>{visible}</span>
      {copyable && (
        <span className="text-ink-fg-faint group-hover:text-ink-accent transition-colors duration-ds-fast">
          {copied ? <CheckIcon /> : <CopyIcon />}
        </span>
      )}
    </span>
  );
}

function truncateAddress(addr: string, n: number): string {
  if (addr.length <= n * 2 + 3) return addr;
  return `${addr.slice(0, n)}…${addr.slice(-4)}`;
}

function CopyIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <rect x="3" y="3" width="7" height="7" rx="1" stroke="currentColor" strokeWidth="1" />
      <path d="M2 8.5V2.5C2 2.224 2.224 2 2.5 2H8.5" stroke="currentColor" strokeWidth="1" strokeLinecap="round" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <path d="M2.5 6.5L5 9L9.5 3.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/* ── Metric (label + value + delta) ────────────────────────────── */

interface MetricProps {
  label: string;
  value: ReactNode;
  unit?: string;
  delta?: { value: string; direction: 'up' | 'down' | 'flat' };
  hint?: string;
  className?: string;
  size?: 'sm' | 'md' | 'lg';
}

export function Metric({ label, value, unit, delta, hint, size = 'md', className }: MetricProps) {
  const valueSize = size === 'sm' ? 'text-lg' : size === 'lg' ? 'text-3xl' : 'text-2xl';
  const deltaColor = delta?.direction === 'up'
    ? 'text-ink-success'
    : delta?.direction === 'down'
      ? 'text-ink-danger'
      : 'text-ink-fg-muted';
  const arrow = delta?.direction === 'up' ? '↑' : delta?.direction === 'down' ? '↓' : '→';

  return (
    <div className={clsx('flex flex-col gap-1', className)}>
      <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">{label}</span>
      <span className="flex items-baseline gap-2">
        <span className={clsx('font-mono font-medium text-ink-fg tabular-nums', valueSize)}>{value}</span>
        {unit && <span className="font-mono text-sm text-ink-fg-secondary">{unit}</span>}
      </span>
      {(delta || hint) && (
        <span className="flex items-center gap-2 text-xs">
          {delta && (
            <span className={clsx('font-mono', deltaColor)}>
              {arrow} {delta.value}
            </span>
          )}
          {hint && <span className="text-ink-fg-muted">{hint}</span>}
        </span>
      )}
    </div>
  );
}

/* ── Stat (compact label-value pair) ───────────────────────────── */

interface StatProps {
  label: string;
  value: ReactNode;
  mono?: boolean;
  className?: string;
}

export function Stat({ label, value, mono = true, className }: StatProps) {
  return (
    <div className={clsx('flex justify-between items-baseline gap-4 py-2 border-b border-ink-border last:border-b-0', className)}>
      <span className="text-xs text-ink-fg-muted">{label}</span>
      <span className={clsx('text-sm text-ink-fg', mono && 'font-mono')}>{value}</span>
    </div>
  );
}

/* ── Skeleton ──────────────────────────────────────────────────── */

interface SkeletonProps {
  width?: string | number;
  height?: string | number;
  className?: string;
}

export function Skeleton({ width = '100%', height = '1em', className }: SkeletonProps) {
  return (
    <span
      className={clsx(
        'inline-block rounded-ds-xs animate-shimmer',
        'bg-[linear-gradient(90deg,var(--color-surface)_0%,var(--color-surface-raised)_50%,var(--color-surface)_100%)]',
        'bg-[length:200%_100%]',
        className,
      )}
      style={{ width, height }}
      aria-hidden
    />
  );
}
