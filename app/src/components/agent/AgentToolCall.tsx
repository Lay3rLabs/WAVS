import { useState } from 'react';
import { useAgentStore } from '../../stores/agentStore';

interface AgentToolCallProps {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  streaming?: boolean;
  rawArgs?: string;
}

function StatusIndicator({ status }: { status: 'running' | 'complete' | 'error' }) {
  if (status === 'running') {
    return (
      <span className="inline-block w-2.5 h-2.5 rounded-full bg-purple-1 animate-pulse" title="Running" />
    );
  }
  if (status === 'complete') {
    return <span className="text-green-400 text-xs" title="Complete">✓</span>;
  }
  return <span className="text-red-3 text-xs" title="Error">✕</span>;
}

/** Format tool name for display */
function formatToolName(name: string): string {
  return name.replace(/_/g, ' ');
}

/** Format args for compact one-line preview */
function formatArgsPreview(args: Record<string, unknown>): string {
  const entries = Object.entries(args);
  if (entries.length === 0) return '';
  // Show first arg value, truncated
  const [, val] = entries[0];
  const str = typeof val === 'string' ? val : JSON.stringify(val);
  return str.length > 60 ? str.slice(0, 57) + '…' : str;
}

export function AgentToolCall({ id, name, arguments: args, streaming, rawArgs }: AgentToolCallProps) {
  const execution = useAgentStore((s) => s.toolExecutions.get(id));
  const status = execution?.status ?? (streaming ? 'running' : 'complete');
  const [manualExpanded, setManualExpanded] = useState<boolean | null>(null);
  // Auto-expand while running; collapse when complete. User click overrides.
  const expanded = manualExpanded ?? status === 'running';

  // Determine what to show for arguments
  const hasArgs = Object.keys(args).length > 0 || (rawArgs && rawArgs.length > 0);
  const argsDisplay = Object.keys(args).length > 0
    ? JSON.stringify(args, null, 2)
    : rawArgs ?? '';

  // Determine result content
  const resultText = execution?.result?.content
    ?.filter((c) => c.type === 'text')
    .map((c) => c.text)
    .join('\n');

  return (
    <div className="my-1.5 rounded-md border border-charcoal-medium overflow-hidden bg-charcoal-darkest/50">
      {/* Header — always visible */}
      <button
        onClick={() => setManualExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 text-left hover:bg-charcoal-medium/30 transition-colors"
      >
        <StatusIndicator status={status} />
        <span className="text-xs font-mono text-purple-1">{formatToolName(name)}</span>
        {!expanded && hasArgs && (
          <span className="text-xs text-tan-muted truncate flex-1 font-mono">
            {formatArgsPreview(args)}
          </span>
        )}
        <svg
          viewBox="0 0 16 16"
          fill="currentColor"
          width="10"
          height="10"
          className={`text-tan-muted transition-transform flex-shrink-0 ${expanded ? 'rotate-180' : ''}`}
        >
          <path d="M4 6l4 4 4-4z" />
        </svg>
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="border-t border-charcoal-medium">
          {/* Arguments */}
          {hasArgs && (
            <div className="px-2.5 py-2">
              <div className="text-[10px] uppercase tracking-wider text-tan-muted mb-1">Arguments</div>
              <pre className="text-xs text-beige-warm whitespace-pre-wrap break-all font-mono leading-relaxed max-h-40 overflow-y-auto">
                {argsDisplay}
                {streaming && !Object.keys(args).length && (
                  <span className="inline-block w-1.5 h-3 ml-0.5 bg-purple-1/60 animate-pulse align-text-bottom" />
                )}
              </pre>
            </div>
          )}

          {/* Result */}
          {resultText && (
            <div className="px-2.5 py-2 border-t border-charcoal-medium/50">
              <div className={`text-[10px] uppercase tracking-wider mb-1 ${
                execution?.result?.isError ? 'text-red-3' : 'text-tan-muted'
              }`}>
                {execution?.result?.isError ? 'Error' : 'Result'}
              </div>
              <pre className={`text-xs whitespace-pre-wrap break-all font-mono leading-relaxed max-h-48 overflow-y-auto ${
                execution?.result?.isError ? 'text-red-3/80' : 'text-beige-warm/80'
              }`}>
                {resultText}
              </pre>
            </div>
          )}

          {/* Streaming indicator when running with no result yet */}
          {status === 'running' && !resultText && (
            <div className="px-2.5 py-2 border-t border-charcoal-medium/50">
              <div className="flex items-center gap-2 text-xs text-tan-muted">
                <span className="inline-block w-2 h-2 rounded-full bg-purple-1 animate-pulse" />
                Running…
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
