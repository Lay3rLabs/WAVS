import { clsx } from 'clsx';

interface CloseXProps {
  onClick: () => void;
  size?: 'sm' | 'lg';
  className?: string;
}

export function CloseX({ onClick, size = 'lg', className }: CloseXProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={clsx(
        'text-tan-muted hover:text-cream-light transition-colors cursor-pointer',
        size === 'sm' ? 'text-lg' : 'text-2xl',
        className
      )}
      aria-label="Close"
    >
      &times;
    </button>
  );
}
