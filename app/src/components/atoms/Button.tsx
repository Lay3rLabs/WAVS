import { clsx } from 'clsx';
import type { ReactNode, MouseEvent } from 'react';

export type ButtonSize = 'sm' | 'lg' | 'xlg';
export type ButtonColor = 'primary' | 'red' | 'purple';
export type ButtonStyle = 'solid' | 'outline';

interface ButtonProps {
  children?: ReactNode;
  text?: string;
  size?: ButtonSize;
  color?: ButtonColor;
  variant?: ButtonStyle;
  disabled?: boolean;
  selected?: boolean;
  onClick?: (e: MouseEvent<HTMLButtonElement>) => void;
  className?: string;
  contentBefore?: ReactNode;
  contentAfter?: ReactNode;
}

const sizeClasses: Record<ButtonSize, string> = {
  sm: 'px-6 py-2 text-sm',
  lg: 'px-8 py-2.5 text-base',
  xlg: 'px-10 py-3 text-lg',
};

const solidColorClasses: Record<ButtonColor, { normal: string; hover: string; selected: string }> = {
  primary: {
    normal: 'bg-charcoal-light text-beige-warm',
    hover: 'hover:bg-charcoal-medium',
    selected: 'bg-charcoal-medium border border-tan-muted',
  },
  red: {
    normal: 'bg-red-2 text-cream-light',
    hover: 'hover:bg-red-3',
    selected: 'bg-red-3 border border-red-4',
  },
  purple: {
    normal: 'bg-purple-1 text-cream-light',
    hover: 'hover:bg-purple-2',
    selected: 'bg-purple-2 border border-purple-3',
  },
};

const outlineColorClasses: Record<ButtonColor, { normal: string; hover: string; selected: string }> = {
  primary: {
    normal: 'border border-charcoal-light text-beige-warm bg-transparent',
    hover: 'hover:border-tan-muted hover:text-cream-light',
    selected: 'border-tan-warm text-cream-light',
  },
  red: {
    normal: 'border border-red-2 text-red-3 bg-transparent',
    hover: 'hover:border-red-3 hover:text-red-4',
    selected: 'border-red-4 text-red-4',
  },
  purple: {
    normal: 'border border-purple-1 text-purple-2 bg-transparent',
    hover: 'hover:border-purple-2 hover:text-purple-3',
    selected: 'border-purple-3 text-purple-3',
  },
};

export function Button({
  children,
  text,
  size = 'lg',
  color = 'primary',
  variant = 'solid',
  disabled = false,
  selected = false,
  onClick,
  className,
  contentBefore,
  contentAfter,
}: ButtonProps) {
  const colorClasses = variant === 'solid' ? solidColorClasses : outlineColorClasses;
  const colorSet = colorClasses[color];

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={clsx(
        'inline-flex items-center justify-center gap-2 rounded-button font-medium transition-colors select-none',
        sizeClasses[size],
        disabled
          ? 'bg-charcoal-medium text-tan-muted cursor-not-allowed opacity-60'
          : selected
            ? colorSet.selected
            : clsx(colorSet.normal, colorSet.hover, 'cursor-pointer'),
        className
      )}
    >
      {contentBefore && <span className="flex shrink-0 items-center leading-none">{contentBefore}</span>}
      {text || children}
      {contentAfter && <span className="flex shrink-0 items-center leading-none">{contentAfter}</span>}
    </button>
  );
}
