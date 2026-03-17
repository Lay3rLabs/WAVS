import { useState, type ReactNode } from 'react';
import { clsx } from 'clsx';

interface ExpanderProps {
  label: ReactNode;
  children: ReactNode;
  defaultExpanded?: boolean;
  className?: string;
}

export function Expander({
  label,
  children,
  defaultExpanded = false,
  className,
}: ExpanderProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div
      className={clsx(
        'p-4 rounded-md bg-charcoal-medium border border-charcoal-light text-beige-warm',
        className
      )}
    >
      {/* Label/toggle */}
      <div
        className="flex items-center gap-2 cursor-pointer select-none"
        onClick={() => setExpanded(!expanded)}
      >
        <span className="text-base">{label}</span>
        <span>{expanded ? '▼' : '▶'}</span>
      </div>

      {/* Content */}
      <div
        className={clsx(
          'mt-4 p-4 rounded-md bg-charcoal-medium border border-charcoal-light text-beige-warm overflow-auto',
          expanded ? 'block' : 'hidden'
        )}
      >
        {children}
      </div>
    </div>
  );
}
