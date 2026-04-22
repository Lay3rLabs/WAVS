import { useEffect, useRef, useState, useCallback } from 'react';
import { useAgentStore, type PendingMessage } from '../../stores/agentStore';
import { AgentMessage } from './AgentMessage';
import { AgentInput } from './AgentInput';
import { AgentUIDialog } from './AgentUIDialog';
import { Button } from '../atoms';
import type { SessionInfo } from '../../tauri/agent';

function StreamingDot() {
  return (
    <span className="inline-block w-2 h-2 rounded-full bg-purple-1 animate-pulse" title="Streaming" />
  );
}

function formatRelativeTime(isoDate: string): string {
  const date = new Date(isoDate);
  const now = Date.now();
  const diffMs = now - date.getTime();
  const diffMin = Math.floor(diffMs / 60_000);
  const diffHr = Math.floor(diffMs / 3_600_000);
  const diffDay = Math.floor(diffMs / 86_400_000);

  if (diffMin < 1) return 'just now';
  if (diffMin < 60) return `${diffMin}m ago`;
  if (diffHr < 24) return `${diffHr}h ago`;
  if (diffDay < 7) return `${diffDay}d ago`;
  return date.toLocaleDateString();
}

function SessionSelector({ onClose }: { onClose: () => void }) {
  const sessions = useAgentStore((s) => s.sessions);
  const currentSessionId = useAgentStore((s) => s.currentSessionId);
  const switchSession = useAgentStore((s) => s.switchSession);
  const newSession = useAgentStore((s) => s.newSession);
  const refreshSessions = useAgentStore((s) => s.refreshSessions);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    refreshSessions();
  }, [refreshSessions]);

  // Close on click outside
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [onClose]);

  return (
    <div
      ref={dropdownRef}
      className="absolute top-full left-0 right-0 z-50 mt-0.5 bg-charcoal-darkest border border-charcoal-medium rounded-b-lg shadow-xl max-h-64 overflow-y-auto"
    >
      {/* New session button */}
      <button
        onClick={() => { newSession(); onClose(); }}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs text-purple-1 hover:bg-charcoal-medium transition-colors border-b border-charcoal-medium"
      >
        <svg viewBox="0 0 16 16" fill="currentColor" width="10" height="10">
          <path d="M8 1v6h6v2H8v6H6V9H0V7h6V1h2z" />
        </svg>
        New session
      </button>

      {sessions.length === 0 && (
        <div className="px-3 py-3 text-xs text-tan-muted text-center">No saved sessions</div>
      )}

      {sessions.map((session: SessionInfo) => {
        const isCurrent = session.id === currentSessionId;
        return (
          <button
            key={session.id}
            onClick={() => {
              if (!isCurrent) {
                switchSession(session.path);
              }
              onClose();
            }}
            className={`w-full text-left px-3 py-2 hover:bg-charcoal-medium transition-colors border-b border-charcoal-medium/50 last:border-0 ${
              isCurrent ? 'bg-charcoal-medium/50' : ''
            }`}
          >
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs text-beige-warm truncate flex-1">
                {session.name || session.first_message}
              </span>
              <span className="text-[10px] text-tan-muted whitespace-nowrap">
                {session.message_count} msg{session.message_count !== 1 ? 's' : ''}
              </span>
            </div>
            <div className="text-[10px] text-tan-muted mt-0.5">
              {formatRelativeTime(session.modified)}
              {isCurrent && <span className="ml-1 text-purple-1">● current</span>}
            </div>
          </button>
        );
      })}
    </div>
  );
}

function formatModelName(_provider: string, model: string): string {
  // Shorten common model names
  const short = model
    .replace('claude-sonnet-4-20250514', 'Sonnet 4')
    .replace('claude-opus-4-20250514', 'Opus 4')
    .replace('claude-haiku-3-5-20241022', 'Haiku 3.5')
    .replace(/^claude-/, '')
    .replace(/-\d{8}$/, '');
  return short;
}

function formatTokenCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function ModelUsageBadge() {
  const modelInfo = useAgentStore((s) => s.modelInfo);
  const usageInfo = useAgentStore((s) => s.usageInfo);

  if (!modelInfo) return null;

  const modelName = formatModelName(modelInfo.provider, modelInfo.model);
  const usagePct = usageInfo ? Math.min(100, Math.round((usageInfo.totalTokens / usageInfo.contextWindow) * 100)) : 0;
  const costStr = usageInfo && usageInfo.cost > 0 ? `$${usageInfo.cost.toFixed(4)}` : null;

  return (
    <div className="flex items-center gap-1.5 text-[10px] text-tan-muted" title={
      usageInfo
        ? `${formatTokenCount(usageInfo.inputTokens)} in / ${formatTokenCount(usageInfo.outputTokens)} out / ${formatTokenCount(usageInfo.totalTokens)} total (${usagePct}% of ${formatTokenCount(usageInfo.contextWindow)} context)${costStr ? ` — ${costStr}` : ''}`
        : modelInfo.model
    }>
      <span className="text-beige-warm/70">{modelName}</span>
      {usageInfo && usageInfo.totalTokens > 0 && (
        <>
          <span className="text-charcoal-light">·</span>
          <span className={usagePct > 80 ? 'text-amber-400' : usagePct > 95 ? 'text-red-3' : ''}>{usagePct}%</span>
          {costStr && (
            <>
              <span className="text-charcoal-light">·</span>
              <span>{costStr}</span>
            </>
          )}
        </>
      )}
    </div>
  );
}

function PanelHeader() {
  const isStreaming = useAgentStore((s) => s.isStreaming);
  const status = useAgentStore((s) => s.status);
  const togglePanel = useAgentStore((s) => s.togglePanel);
  const startAgent = useAgentStore((s) => s.startAgent);
  const abort = useAgentStore((s) => s.abort);
  const [showSessions, setShowSessions] = useState(false);

  return (
    <div className="relative">
      <div className="flex items-center justify-between px-3 py-2 border-b border-charcoal-medium bg-charcoal-dark">
        <div className="flex items-center gap-2">
          {/* Session selector trigger */}
          <button
            onClick={() => setShowSessions(!showSessions)}
            className="flex items-center gap-1 text-sm font-medium text-beige-warm hover:text-cream-light transition-colors"
            title="Switch session"
          >
            Agent
            <svg viewBox="0 0 16 16" fill="currentColor" width="10" height="10" className={`transition-transform ${showSessions ? 'rotate-180' : ''}`}>
              <path d="M4 6l4 4 4-4z" />
            </svg>
          </button>
          {isStreaming && <StreamingDot />}
          {status === 'error' && (
            <span
              className="text-xs text-red-3 cursor-pointer hover:text-red-2 transition-colors"
              onClick={() => startAgent()}
              title="Click to retry"
            >
              Error — retry
            </span>
          )}
          {status === 'stopped' && (
            <span className="text-xs text-tan-muted">Starting…</span>
          )}
        </div>

        <div className="flex items-center gap-1">
          <ModelUsageBadge />

          {isStreaming && (
            <Button
              text="Stop"
              size="sm"
              color="red"
              className="!px-3 !py-1 text-xs"
              onClick={() => abort()}
            />
          )}

          {/* Collapse panel */}
          <button
            onClick={togglePanel}
            className="p-1.5 rounded text-tan-muted hover:text-beige-warm hover:bg-charcoal-medium transition-colors"
            title="Close panel"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
              <path d="M12.207 4.793a1 1 0 010 1.414L9.414 9l2.793 2.793a1 1 0 01-1.414 1.414l-3.5-3.5a1 1 0 010-1.414l3.5-3.5a1 1 0 011.414 0z" />
              <path d="M7.207 4.793a1 1 0 010 1.414L4.414 9l2.793 2.793a1 1 0 01-1.414 1.414l-3.5-3.5a1 1 0 010-1.414l3.5-3.5a1 1 0 011.414 0z" />
            </svg>
          </button>
        </div>
      </div>

      {showSessions && <SessionSelector onClose={() => setShowSessions(false)} />}
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center px-6">
      <div className="text-4xl mb-3 opacity-30">💬</div>
      <p className="text-sm text-tan-muted">
        Ask the agent anything about WAVS — build components, deploy services, troubleshoot issues.
      </p>
    </div>
  );
}

function PendingMessageIndicator({ pending }: { pending: PendingMessage }) {
  const modeLabel = pending.mode === 'steer' ? 'Interrupt' : 'Follow-up';
  const modeColor = pending.mode === 'steer' ? 'text-amber-400' : 'text-purple-1';
  return (
    <div className="flex items-start gap-2 px-3 py-2 rounded-lg bg-charcoal-medium/30 border border-charcoal-light/30">
      <span className={`text-[10px] font-medium shrink-0 mt-0.5 ${modeColor}`}>{modeLabel}</span>
      <span className="text-xs text-tan-muted line-clamp-2">{pending.text}</span>
    </div>
  );
}

function MessageList() {
  const messages = useAgentStore((s) => s.messages);
  const isStreaming = useAgentStore((s) => s.isStreaming);
  const toolExecutions = useAgentStore((s) => s.toolExecutions);
  const pendingMessages = useAgentStore((s) => s.pendingMessages);
  const scrollRef = useRef<HTMLDivElement>(null);
  const userScrolledUp = useRef(false);

  // Auto-scroll unless user manually scrolled up
  useEffect(() => {
    const el = scrollRef.current;
    if (el && !userScrolledUp.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages, isStreaming, toolExecutions, pendingMessages]);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    // Consider "at bottom" if within 40px of the bottom
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    userScrolledUp.current = !atBottom;
  }, []);

  if (messages.length === 0 && pendingMessages.length === 0) {
    return <EmptyState />;
  }

  return (
    <div ref={scrollRef} onScroll={handleScroll} className="flex-1 overflow-y-auto px-3 py-3 space-y-1">
      {messages.map((msg, i) => (
        <AgentMessage key={i} message={msg} />
      ))}
      {/* Pending messages shown at bottom during streaming */}
      {pendingMessages.length > 0 && (
        <div className="space-y-1 pt-2 border-t border-charcoal-light/20">
          {pendingMessages.map((pm, i) => (
            <PendingMessageIndicator key={i} pending={pm} />
          ))}
        </div>
      )}
    </div>
  );
}

export function AgentPanel() {
  const error = useAgentStore((s) => s.error);

  return (
    <div className="flex flex-col h-full bg-charcoal-dark border-l border-charcoal-medium">
      <PanelHeader />

      {error && (
        <div className="px-3 py-2 bg-red-2/20 border-b border-red-2/40 text-xs text-red-3">
          {error}
        </div>
      )}

      <MessageList />
      <AgentUIDialog />
      <AgentInput />
    </div>
  );
}
