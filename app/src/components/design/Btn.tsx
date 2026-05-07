import { clsx } from 'clsx';
import type { ReactNode, MouseEvent } from 'react';

export type BtnVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type BtnSize = 'sm' | 'md' | 'lg';

interface BtnProps {
  children?: ReactNode;
  variant?: BtnVariant;
  size?: BtnSize;
  disabled?: boolean;
  loading?: boolean;
  leading?: ReactNode;
  trailing?: ReactNode;
  onClick?: (e: MouseEvent<HTMLButtonElement>) => void;
  type?: 'button' | 'submit' | 'reset';
  className?: string;
  fullWidth?: boolean;
  'aria-label'?: string;
}

const sizeClasses: Record<BtnSize, string> = {
  sm: 'h-7  px-2.5 text-xs gap-1.5',
  md: 'h-8  px-3   text-sm gap-2',
  lg: 'h-10 px-4   text-md gap-2',
};

const variantClasses: Record<BtnVariant, string> = {
  primary:
    'bg-ink-accent text-ink-accent-fg ' +
    'hover:bg-ink-accent-hover ' +
    'active:bg-ink-accent-pressed',
  secondary:
    'bg-ink-surface-raised text-ink-fg border border-ink-border ' +
    'hover:bg-ink-surface-overlay hover:border-ink-border-strong',
  ghost:
    'bg-transparent text-ink-fg-secondary border border-transparent ' +
    'hover:text-ink-fg hover:bg-ink-surface',
  danger:
    'bg-ink-danger-tint text-ink-danger border border-ink-danger-edge ' +
    'hover:bg-ink-danger hover:text-ink-fg-inverse',
};

export function Btn({
  children,
  variant = 'secondary',
  size = 'md',
  disabled,
  loading,
  leading,
  trailing,
  onClick,
  type = 'button',
  className,
  fullWidth,
  'aria-label': ariaLabel,
}: BtnProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      onClick={onClick}
      aria-label={ariaLabel}
      aria-busy={loading || undefined}
      className={clsx(
        'inline-flex items-center justify-center font-medium rounded-ds-xs select-none whitespace-nowrap',
        'transition-colors duration-ds-fast ease-ds',
        'cursor-pointer',
        sizeClasses[size],
        variantClasses[variant],
        fullWidth && 'w-full',
        (disabled || loading) && 'opacity-50 cursor-not-allowed pointer-events-none',
        className,
      )}
    >
      {loading ? (
        <span className="inline-block w-3 h-3 rounded-full border border-current border-t-transparent animate-spin" />
      ) : (
        leading
      )}
      {children && <span className="leading-none">{children}</span>}
      {!loading && trailing}
    </button>
  );
}
