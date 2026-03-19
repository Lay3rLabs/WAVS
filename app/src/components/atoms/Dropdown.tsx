import { useState, useRef, useEffect } from 'react';
import { clsx } from 'clsx';

export interface DropdownOption<T> {
  label: string;
  value: T;
}

export type DropdownSize = 'sm' | 'md';

interface DropdownProps<T> {
  options: DropdownOption<T>[];
  value?: T;
  onChange?: (value: T) => void;
  placeholder?: string;
  size?: DropdownSize;
  className?: string;
}

const sizeClasses: Record<DropdownSize, { container: string; option: string }> = {
  sm: { container: 'px-4 py-2 text-sm',    option: 'px-4 py-1.5 text-sm' },
  md: { container: 'px-4 py-2.5 text-base', option: 'px-4 py-2 text-base' },
};

function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      width="14"
      height="14"
      className={clsx('transition-transform duration-150', open && 'rotate-180')}
    >
      <polyline points="4,6 8,10 12,6" />
    </svg>
  );
}

export function Dropdown<T>({
  options,
  value,
  onChange,
  placeholder = 'Select...',
  size = 'md',
  className,
}: DropdownProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const selectedOption = options.find((opt) => opt.value === value);
  const sizeClass = sizeClasses[size];

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleSelect = (option: DropdownOption<T>) => {
    onChange?.(option.value);
    setIsOpen(false);
  };

  return (
    <div ref={containerRef} className={clsx('inline-flex flex-col select-none', className)}>
      <div className="relative inline-block">
        {/* Trigger */}
        <div
          className={clsx(
            'flex justify-between items-center gap-4 rounded border bg-charcoal-dark cursor-pointer transition-colors',
            'text-beige-warm',
            isOpen ? 'border-purple-2' : 'border-charcoal-light hover:border-tan-muted',
            sizeClass.container
          )}
          onClick={() => setIsOpen(!isOpen)}
        >
          <span className={clsx(!selectedOption && 'text-tan-muted')}>
            {selectedOption?.label ?? placeholder}
          </span>
          <span className="text-tan-muted">
            <ChevronIcon open={isOpen} />
          </span>
        </div>

        {/* Options panel */}
        {isOpen && (
          <div className="absolute top-full left-0 w-full min-w-max z-50 mt-1 bg-charcoal-dark border border-charcoal-light rounded shadow-[8px_8px_20px_rgba(0,0,0,0.25)]">
            <div className="flex flex-col py-1 max-h-96 overflow-y-auto">
              {options.map((option, index) => (
                <div
                  key={index}
                  className={clsx(
                    'cursor-pointer transition-colors',
                    sizeClass.option,
                    option.value === value
                      ? 'bg-charcoal-medium text-cream-light'
                      : 'text-beige-warm hover:bg-charcoal-medium hover:text-cream-light'
                  )}
                  onClick={() => handleSelect(option)}
                >
                  {option.label}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
