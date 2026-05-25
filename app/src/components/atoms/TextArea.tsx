import { type ChangeEvent } from 'react';
import { clsx } from 'clsx';

interface TextAreaProps {
  placeholder?: string;
  value?: string;
  defaultValue?: string;
  onChange?: (value: string) => void;
  onInput?: (value: string) => void;
  disabled?: boolean;
  readOnly?: boolean;
  rows?: number;
  className?: string;
}

export function TextArea({
  placeholder,
  value,
  defaultValue,
  onChange,
  onInput,
  disabled,
  readOnly,
  rows = 4,
  className,
}: TextAreaProps) {
  const handleChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value;
    onChange?.(newValue);
    onInput?.(newValue);
  };

  return (
    <textarea
      placeholder={placeholder}
      value={value}
      defaultValue={defaultValue}
      onChange={handleChange}
      disabled={disabled}
      readOnly={readOnly}
      rows={rows}
      className={clsx(
        'px-7 py-2.5 rounded border border-charcoal-light bg-charcoal-dark text-beige-warm',
        'outline-none transition-colors focus:border-tan-muted resize-y',
        'placeholder:text-tan-muted',
        disabled && 'opacity-60 cursor-not-allowed',
        readOnly && 'cursor-default',
        className
      )}
    />
  );
}
