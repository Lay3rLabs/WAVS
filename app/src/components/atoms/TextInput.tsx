import { useState, type ChangeEvent } from 'react';
import { clsx } from 'clsx';

export type TextInputKind = 'text' | 'email' | 'password' | 'number';

interface TextInputProps {
  kind?: TextInputKind;
  placeholder?: string;
  value?: string;
  defaultValue?: string;
  onChange?: (value: string) => void;
  onInput?: (value: string) => void;
  disabled?: boolean;
  readOnly?: boolean;
  className?: string;
}

export function TextInput({
  kind = 'text',
  placeholder,
  value,
  defaultValue,
  onChange,
  onInput,
  disabled,
  readOnly,
  className,
}: TextInputProps) {
  const [showPassword, setShowPassword] = useState(false);

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    onChange?.(newValue);
    onInput?.(newValue);
  };

  const inputType = kind === 'password' && showPassword ? 'text' : kind;

  return (
    <div className="flex flex-col gap-2">
      <input
        type={inputType}
        placeholder={placeholder}
        value={value}
        defaultValue={defaultValue}
        onChange={handleChange}
        disabled={disabled}
        readOnly={readOnly}
        autoComplete="off"
        spellCheck="false"
        autoCorrect="off"
        className={clsx(
          'px-7 py-2.5 rounded border border-charcoal-light bg-charcoal-dark text-beige-warm',
          'outline-none transition-colors focus:border-purple-2',
          'placeholder:text-tan-muted',
          disabled && 'opacity-60 cursor-not-allowed',
          readOnly && 'cursor-default',
          className
        )}
      />
      {kind === 'password' && (
        <button
          type="button"
          onClick={() => setShowPassword(!showPassword)}
          className="text-sm text-tan-muted hover:text-cream-light cursor-pointer self-start"
        >
          {showPassword ? 'Hide password' : 'Show password'}
        </button>
      )}
    </div>
  );
}
