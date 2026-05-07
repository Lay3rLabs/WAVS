import { clsx } from 'clsx';
import type { ReactNode } from 'react';

export type NotifyTone = 'info' | 'success' | 'warning' | 'danger' | 'accent' | 'neutral';

const toneStyle: Record<NotifyTone, { bg: string; border: string; fg: string; icon: string }> = {
  info:    { bg: 'bg-ink-info-tint',    border: 'border-ink-info-edge',    fg: 'text-ink-info',    icon: 'text-ink-info' },
  success: { bg: 'bg-ink-success-tint', border: 'border-ink-success-edge', fg: 'text-ink-success', icon: 'text-ink-success' },
  warning: { bg: 'bg-ink-warning-tint', border: 'border-ink-warning-edge', fg: 'text-ink-warning', icon: 'text-ink-warning' },
  danger:  { bg: 'bg-ink-danger-tint',  border: 'border-ink-danger-edge',  fg: 'text-ink-danger',  icon: 'text-ink-danger' },
  accent:  { bg: 'bg-ink-accent-tint',  border: 'border-ink-accent-edge',  fg: 'text-ink-accent',  icon: 'text-ink-accent' },
  neutral: { bg: 'bg-ink-surface-raised', border: 'border-ink-border',     fg: 'text-ink-fg',      icon: 'text-ink-fg-muted' },
};

/* ── Tone glyphs ───────────────────────────────────────────────── */

function ToneGlyph({ tone }: { tone: NotifyTone }) {
  const cls = clsx('shrink-0', toneStyle[tone].icon);
  switch (tone) {
    case 'success':
      return (
        <svg className={cls} width="14" height="14" viewBox="0 0 14 14" fill="none">
          <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.2" />
          <path d="M4.5 7.5l1.7 1.7L9.7 5.7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case 'danger':
      return (
        <svg className={cls} width="14" height="14" viewBox="0 0 14 14" fill="none">
          <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.2" />
          <path d="M7 4v3.5M7 9.6v.4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        </svg>
      );
    case 'warning':
      return (
        <svg className={cls} width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M7 1.5L13 11.5H1L7 1.5Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
          <path d="M7 5.5v3M7 10.2v.3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        </svg>
      );
    case 'info':
    case 'accent':
      return (
        <svg className={cls} width="14" height="14" viewBox="0 0 14 14" fill="none">
          <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.2" />
          <path d="M7 6.5v3.5M7 4v.4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        </svg>
      );
    case 'neutral':
      return (
        <svg className={cls} width="14" height="14" viewBox="0 0 14 14" fill="none">
          <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.2" />
        </svg>
      );
  }
}

/* ── Alert (inline or banner) ──────────────────────────────────── */

interface AlertProps {
  tone?: NotifyTone;
  variant?: 'inline' | 'banner';
  title?: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  onDismiss?: () => void;
  icon?: ReactNode;
  className?: string;
  children?: ReactNode;
}

export function Alert({
  tone = 'info',
  variant = 'inline',
  title,
  description,
  action,
  onDismiss,
  icon,
  className,
  children,
}: AlertProps) {
  const s = toneStyle[tone];
  return (
    <div
      role="alert"
      className={clsx(
        'flex items-start gap-3 border',
        variant === 'inline' ? 'rounded-ds-sm px-4 py-3' : 'px-6 py-3',
        s.bg, s.border,
        className,
      )}
    >
      <span className={clsx('pt-0.5', s.icon)}>{icon ?? <ToneGlyph tone={tone} />}</span>
      <div className="flex-1 min-w-0 flex flex-col gap-1">
        {title && (
          <div className={clsx('text-sm font-medium leading-snug', s.fg)}>{title}</div>
        )}
        {description && (
          <div className="text-sm text-ink-fg-secondary leading-snug">{description}</div>
        )}
        {children}
      </div>
      {action && <div className="shrink-0 self-start">{action}</div>}
      {onDismiss && (
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss"
          className="shrink-0 -mr-1 -mt-0.5 p-1 text-ink-fg-muted hover:text-ink-fg cursor-pointer rounded-ds-xs transition-colors duration-ds-fast"
        >
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
            <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
        </button>
      )}
    </div>
  );
}

/* ── Toast (transient, single instance) ────────────────────────── */

interface ToastProps {
  tone?: NotifyTone;
  title: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  onDismiss?: () => void;
  className?: string;
}

export function Toast({
  tone = 'neutral',
  title,
  description,
  action,
  onDismiss,
  className,
}: ToastProps) {
  const s = toneStyle[tone];
  return (
    <div
      role="status"
      className={clsx(
        'flex items-start gap-3 px-4 py-3 rounded-ds-sm border',
        'bg-ink-surface-overlay border-ink-border-strong',
        'min-w-[320px] max-w-[420px]',
        'shadow-[0_0_0_1px_var(--color-canvas)]',
        'animate-toast-in',
        className,
      )}
    >
      <span className={clsx('pt-0.5', s.icon)}>
        <ToneGlyph tone={tone} />
      </span>
      <div className="flex-1 min-w-0 flex flex-col gap-0.5">
        <div className="text-sm text-ink-fg leading-snug">{title}</div>
        {description && (
          <div className="text-xs text-ink-fg-secondary leading-snug">{description}</div>
        )}
        {action && <div className="mt-2">{action}</div>}
      </div>
      {onDismiss && (
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss"
          className="shrink-0 -mr-1 -mt-0.5 p-1 text-ink-fg-muted hover:text-ink-fg cursor-pointer rounded-ds-xs transition-colors duration-ds-fast"
        >
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
            <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
        </button>
      )}
    </div>
  );
}

/* ── ToastStack — bottom-right viewport ────────────────────────── */

interface ToastStackProps {
  position?: 'br' | 'tr' | 'bl' | 'tl';
  children: ReactNode;
  className?: string;
}

export function ToastStack({ position = 'br', children, className }: ToastStackProps) {
  const pos = {
    br: 'bottom-4 right-4 items-end',
    tr: 'top-4 right-4 items-end',
    bl: 'bottom-4 left-4 items-start',
    tl: 'top-4 left-4 items-start',
  }[position];
  return (
    <div className={clsx('pointer-events-none fixed z-50 flex flex-col gap-2', pos, className)}>
      <div className="pointer-events-auto flex flex-col gap-2">{children}</div>
    </div>
  );
}
