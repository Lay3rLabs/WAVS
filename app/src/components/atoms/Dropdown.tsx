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

const sizeClasses: Record<DropdownSize, { container: string; options: string }> = {
  sm: { container: 'p-2 text-sm', options: 'p-2' },
  md: { container: 'p-4 text-base', options: 'p-4' },
};

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

  // Close dropdown when clicking outside
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
    <div ref={containerRef} className={clsx('inline-flex flex-col gap-4 select-none', className)}>
      <div className="relative inline-block border border-charcoal-light rounded bg-charcoal-dark cursor-pointer">
        {/* Selected value display */}
        <div
          className={clsx(
            'flex justify-between items-center gap-4 text-beige-warm',
            sizeClass.container
          )}
          onClick={() => setIsOpen(!isOpen)}
        >
          <span>{selectedOption?.label ?? placeholder}</span>
          <span>{isOpen ? '▲' : '▼'}</span>
        </div>

        {/* Options dropdown */}
        {isOpen && (
          <div className="absolute top-full left-0 w-max z-50 bg-charcoal-dark border border-charcoal-light rounded mt-1">
            <div
              className={clsx(
                'flex flex-col gap-2 max-h-96 overflow-y-auto',
                sizeClass.options
              )}
            >
              {options.map((option, index) => (
                <div
                  key={index}
                  className="text-beige-warm hover:text-cream-light cursor-pointer transition-colors"
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
