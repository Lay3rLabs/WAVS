import React, { useState, useCallback, useRef, useEffect, type KeyboardEvent } from 'react';
import { useAgentStore } from '../../stores/agentStore';

type StreamingSendMode = 'steer' | 'followUp';

function SteerIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
      <path d="M8 2L3 8h3.5v6h3V8H13L8 2z" />
    </svg>
  );
}

function FollowUpIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12">
      <path d="M2 4h9v2H2zM2 8h12v2H2zM2 12h7v2H2z" />
    </svg>
  );
}

const MODE_CONFIG: Record<StreamingSendMode, { label: string; title: string; icon: () => React.JSX.Element }> = {
  steer: {
    label: 'Interrupt',
    title: 'Steer — interrupt the agent mid-turn and redirect',
    icon: SteerIcon,
  },
  followUp: {
    label: 'Follow-up',
    title: 'Follow-up — queue message for after the current turn',
    icon: FollowUpIcon,
  },
};

const MAX_TEXTAREA_HEIGHT = 160; // ~8 lines

export function AgentInput() {
  const [text, setText] = useState('');
  const [sendMode, setSendMode] = useState<StreamingSendMode>('followUp');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const isStreaming = useAgentStore((s) => s.isStreaming);
  const status = useAgentStore((s) => s.status);
  const sendMessage = useAgentStore((s) => s.sendMessage);
  const abort = useAgentStore((s) => s.abort);

  const hasText = text.trim().length > 0;
  const canSend = hasText && status === 'running';

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, MAX_TEXTAREA_HEIGHT)}px`;
  }, [text]);

  const handleSend = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || status !== 'running') return;
    sendMessage(trimmed, isStreaming ? sendMode : undefined);
    setText('');
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  }, [text, status, isStreaming, sendMode, sendMessage]);

  const toggleMode = useCallback(() => {
    setSendMode((m) => (m === 'steer' ? 'followUp' : 'steer'));
  }, []);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const placeholder = status !== 'running'
    ? 'Start agent to chat…'
    : isStreaming
      ? sendMode === 'steer'
        ? 'Interrupt and redirect…'
        : 'Queue for after this turn…'
      : 'Ask the agent…';

  const modeConfig = MODE_CONFIG[sendMode];
  const ModeIcon = modeConfig.icon;

  return (
    <div className="flex flex-col border-t border-charcoal-medium bg-charcoal-dark">
      <div className="flex items-end gap-2 p-3">
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={status !== 'running'}
          rows={1}
          className="flex-1 resize-none px-3 py-2 rounded-lg border border-charcoal-light bg-charcoal-darkest text-beige-warm text-sm
            outline-none transition-colors focus:border-tan-muted placeholder:text-tan-muted
            disabled:opacity-50 disabled:cursor-not-allowed
            overflow-y-auto"
          style={{ minHeight: '2.25rem', maxHeight: `${MAX_TEXTAREA_HEIGHT}px` }}
        />

        {isStreaming ? (
          <div className="flex items-end gap-1">
            {/* Mode toggle + send button */}
            <div className="flex rounded-lg overflow-hidden">
              {/* Mode toggle */}
              <button
                onClick={toggleMode}
                className="flex items-center gap-1 px-2 py-2 bg-charcoal-medium text-tan-muted text-xs font-medium
                  hover:text-beige-warm hover:bg-charcoal-light transition-colors cursor-pointer border-r border-charcoal-dark"
                title={`Switch to ${sendMode === 'steer' ? 'follow-up' : 'interrupt'} mode`}
              >
                <ModeIcon />
                <span>{modeConfig.label}</span>
              </button>

              {/* Send button */}
              <button
                onClick={handleSend}
                disabled={!canSend}
                className="flex-shrink-0 px-3 py-2 bg-purple-1 text-cream-light text-sm font-medium
                  hover:bg-purple-2 transition-colors cursor-pointer
                  disabled:bg-charcoal-medium disabled:text-tan-muted disabled:cursor-not-allowed"
                title={modeConfig.title}
              >
                ↑
              </button>
            </div>

            {/* Abort button */}
            <button
              onClick={() => abort()}
              className="flex-shrink-0 px-3 py-2 rounded-lg bg-red-2 text-cream-light text-sm font-medium
                hover:bg-red-3 transition-colors cursor-pointer"
              title="Abort"
            >
              ■
            </button>
          </div>
        ) : (
          <button
            onClick={handleSend}
            disabled={!canSend}
            className="flex-shrink-0 px-3 py-2 rounded-lg bg-purple-1 text-cream-light text-sm font-medium
              hover:bg-purple-2 transition-colors cursor-pointer
              disabled:bg-charcoal-medium disabled:text-tan-muted disabled:cursor-not-allowed"
            title="Send"
          >
            ↑
          </button>
        )}
      </div>
    </div>
  );
}
