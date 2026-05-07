import { clsx } from 'clsx';
import type { ReactNode, ChangeEvent, KeyboardEvent } from 'react';

/* ── Label / Help / Field wrapper ──────────────────────────────── */

interface FieldProps {
  label?: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  optional?: boolean;
  children: ReactNode;
  className?: string;
  id?: string;
}

export function Field({ label, hint, error, optional, children, className, id }: FieldProps) {
  return (
    <div className={clsx('flex flex-col gap-1.5', className)}>
      {label && (
        <label htmlFor={id} className="flex items-baseline justify-between text-xs font-medium text-ink-fg-secondary uppercase tracking-wider">
          <span>{label}</span>
          {optional && <span className="text-ink-fg-faint normal-case tracking-normal">optional</span>}
        </label>
      )}
      {children}
      {error ? (
        <p className="text-xs text-ink-danger">{error}</p>
      ) : hint ? (
        <p className="text-xs text-ink-fg-muted">{hint}</p>
      ) : null}
    </div>
  );
}

/* ── Input ─────────────────────────────────────────────────────── */

interface InputProps {
  type?: 'text' | 'email' | 'password' | 'number' | 'search';
  value?: string;
  defaultValue?: string;
  placeholder?: string;
  disabled?: boolean;
  readOnly?: boolean;
  invalid?: boolean;
  mono?: boolean;
  leading?: ReactNode;
  trailing?: ReactNode;
  onChange?: (value: string) => void;
  onKeyDown?: (e: KeyboardEvent<HTMLInputElement>) => void;
  onBlur?: () => void;
  onFocus?: () => void;
  className?: string;
  id?: string;
  autoFocus?: boolean;
}

export function Input({
  type = 'text',
  value,
  defaultValue,
  placeholder,
  disabled,
  readOnly,
  invalid,
  mono,
  leading,
  trailing,
  onChange,
  onKeyDown,
  onBlur,
  onFocus,
  className,
  id,
  autoFocus,
}: InputProps) {
  const handleChange = (e: ChangeEvent<HTMLInputElement>) => onChange?.(e.target.value);

  return (
    <div
      className={clsx(
        'flex items-center gap-2 h-8 px-2.5 rounded-ds-xs',
        'border bg-ink-surface',
        'transition-colors duration-ds-fast ease-ds',
        invalid
          ? 'border-ink-danger-edge focus-within:border-ink-danger'
          : 'border-ink-border focus-within:border-ink-accent',
        disabled && 'opacity-50',
        readOnly && 'bg-ink-surface-sunken',
        className,
      )}
    >
      {leading && <span className="text-ink-fg-muted shrink-0">{leading}</span>}
      <input
        id={id}
        type={type}
        value={value}
        defaultValue={defaultValue}
        placeholder={placeholder}
        disabled={disabled}
        readOnly={readOnly}
        autoFocus={autoFocus}
        onChange={handleChange}
        onKeyDown={onKeyDown}
        onBlur={onBlur}
        onFocus={onFocus}
        autoComplete="off"
        spellCheck="false"
        className={clsx(
          'flex-1 min-w-0 bg-transparent outline-none',
          'text-sm text-ink-fg placeholder:text-ink-fg-faint',
          mono && 'font-mono',
          (disabled || readOnly) && 'cursor-not-allowed',
        )}
      />
      {trailing && <span className="text-ink-fg-muted shrink-0">{trailing}</span>}
    </div>
  );
}

/* ── Textarea ──────────────────────────────────────────────────── */

interface TextareaProps {
  value?: string;
  defaultValue?: string;
  placeholder?: string;
  disabled?: boolean;
  rows?: number;
  mono?: boolean;
  invalid?: boolean;
  onChange?: (value: string) => void;
  className?: string;
  id?: string;
}

export function Textarea({
  value, defaultValue, placeholder, disabled, rows = 4, mono, invalid, onChange, className, id,
}: TextareaProps) {
  return (
    <textarea
      id={id}
      value={value}
      defaultValue={defaultValue}
      placeholder={placeholder}
      disabled={disabled}
      rows={rows}
      onChange={(e) => onChange?.(e.target.value)}
      spellCheck="false"
      className={clsx(
        'w-full px-2.5 py-2 rounded-ds-xs resize-y',
        'border bg-ink-surface text-ink-fg placeholder:text-ink-fg-faint',
        'outline-none transition-colors duration-ds-fast ease-ds',
        'text-sm',
        invalid
          ? 'border-ink-danger-edge focus:border-ink-danger'
          : 'border-ink-border focus:border-ink-accent',
        mono && 'font-mono',
        disabled && 'opacity-50 cursor-not-allowed',
        className,
      )}
    />
  );
}

/* ── Select ────────────────────────────────────────────────────── */

interface SelectProps {
  value?: string;
  options: { value: string; label: string }[];
  onChange?: (value: string) => void;
  disabled?: boolean;
  className?: string;
  id?: string;
}

export function Select({ value, options, onChange, disabled, className, id }: SelectProps) {
  return (
    <div
      className={clsx(
        'relative flex items-center h-8 rounded-ds-xs border bg-ink-surface',
        'border-ink-border focus-within:border-ink-accent',
        'transition-colors duration-ds-fast ease-ds',
        disabled && 'opacity-50',
        className,
      )}
    >
      <select
        id={id}
        value={value}
        onChange={(e) => onChange?.(e.target.value)}
        disabled={disabled}
        className="appearance-none w-full h-full pl-2.5 pr-7 bg-transparent text-sm text-ink-fg outline-none cursor-pointer"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value} className="bg-ink-surface text-ink-fg">
            {opt.label}
          </option>
        ))}
      </select>
      <svg width="10" height="10" viewBox="0 0 10 10" className="absolute right-2.5 pointer-events-none text-ink-fg-muted">
        <path d="M2 4l3 3 3-3" stroke="currentColor" strokeWidth="1.2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

/* ── Toggle ────────────────────────────────────────────────────── */

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  description?: string;
}

export function Toggle({ checked, onChange, disabled, label, description }: ToggleProps) {
  const toggle = (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={clsx(
        'relative inline-flex h-4 w-7 shrink-0 rounded-ds-pill border transition-colors duration-ds-fast ease-ds cursor-pointer',
        checked
          ? 'bg-ink-accent border-ink-accent'
          : 'bg-ink-surface-raised border-ink-border',
        disabled && 'opacity-50 cursor-not-allowed',
      )}
    >
      <span
        aria-hidden
        className={clsx(
          'absolute top-0.5 h-3 w-3 rounded-ds-pill bg-ink-bg transition-transform duration-ds-fast ease-ds',
          checked ? 'translate-x-3.5' : 'translate-x-0.5',
        )}
      />
    </button>
  );

  if (!label) return toggle;
  return (
    <label className={clsx('flex items-start gap-3', !disabled && 'cursor-pointer')}>
      {toggle}
      <span className="flex flex-col gap-0.5">
        <span className="text-sm text-ink-fg leading-tight">{label}</span>
        {description && <span className="text-xs text-ink-fg-muted leading-snug">{description}</span>}
      </span>
    </label>
  );
}
