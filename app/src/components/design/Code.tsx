import { clsx } from 'clsx';
import { useState, type ReactNode } from 'react';

/* ── Inline code ───────────────────────────────────────────────── */

export function Code({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <code className={clsx(
      'inline px-1 py-px font-mono text-[0.92em]',
      'bg-ink-surface-sunken text-ink-fg-secondary',
      'border border-ink-border rounded-ds-xs',
      className,
    )}>
      {children}
    </code>
  );
}

/* ── Code block ────────────────────────────────────────────────── */

interface CodeBlockProps {
  language?: string;
  children: string;
  copyable?: boolean;
  className?: string;
}

export function CodeBlock({ language, children, copyable = true, className }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(children);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch { /* ignore */ }
  };

  return (
    <div className={clsx(
      'relative group rounded-ds-sm bg-ink-surface-sunken border border-ink-border overflow-hidden',
      className,
    )}>
      {(language || copyable) && (
        <div className="flex items-center justify-between px-3 py-1.5 border-b border-ink-border">
          {language && (
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">{language}</span>
          )}
          {copyable && (
            <button
              type="button"
              onClick={copy}
              className="font-mono text-xs text-ink-fg-muted hover:text-ink-fg cursor-pointer transition-colors duration-ds-fast"
            >
              {copied ? 'copied' : 'copy'}
            </button>
          )}
        </div>
      )}
      <pre className="px-3 py-2.5 overflow-x-auto">
        <code className="font-mono text-xs text-ink-fg-secondary leading-relaxed">{children}</code>
      </pre>
    </div>
  );
}
