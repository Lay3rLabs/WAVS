import { useState, useRef, useEffect } from 'react';
import { clsx } from 'clsx';

export interface MenuOption {
  label: string;
  onClick: () => void;
  variant?: 'default' | 'danger';
}

interface DropdownMenuProps {
  label: string;
  options: MenuOption[];
  size?: 'sm' | 'lg';
}

const sizeClasses = {
  sm: 'px-3 py-1.5 text-sm',
  lg: 'px-4 py-2 text-base',
};

export function DropdownMenu({ label, options, size = 'sm' }: DropdownMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  return (
    <div ref={containerRef} className="relative inline-block">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={clsx(
          'rounded border border-charcoal-light bg-charcoal-dark text-beige-warm hover:bg-charcoal-medium transition-colors',
          sizeClasses[size],
        )}
      >
        {label} {isOpen ? '▲' : '▼'}
      </button>

      {isOpen && (
        <div className="absolute right-0 top-full mt-1 z-50 min-w-[180px] bg-charcoal-dark border border-charcoal-light rounded shadow-lg">
          <div className="py-1">
            {options.map((option, i) => (
              <button
                key={i}
                type="button"
                onClick={() => {
                  option.onClick();
                  setIsOpen(false);
                }}
                className={clsx(
                  'w-full text-left px-4 py-2 text-sm transition-colors',
                  option.variant === 'danger'
                    ? 'text-red-3 hover:bg-charcoal-medium'
                    : 'text-beige-warm hover:bg-charcoal-medium hover:text-cream-light',
                )}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
