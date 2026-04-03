import { useState } from 'react';
import { useAgentStore } from '../../stores/agentStore';
import { Button } from '../atoms';

/**
 * Renders extension UI dialog requests (select, confirm, input, editor).
 * When the agent's extensions call ctx.ui.select/confirm/input/editor,
 * pi emits an extension_ui_request and blocks until we respond.
 */
export function AgentUIDialog() {
  const request = useAgentStore((s) => s.pendingUIRequest);
  const respond = useAgentStore((s) => s.respondToUIRequest);
  const [inputValue, setInputValue] = useState('');

  if (!request) return null;

  const cancel = () => respond(request.id, { cancelled: true });

  switch (request.method) {
    case 'select':
      return (
        <div className="mx-3 mb-3 p-3 rounded-lg border border-purple-1/40 bg-charcoal-darkest">
          <p className="text-sm text-beige-warm mb-2">{request.title}</p>
          <div className="flex flex-col gap-1">
            {(request.options ?? []).map((opt) => (
              <button
                key={opt}
                onClick={() => respond(request.id, { value: opt })}
                className="text-left px-3 py-1.5 rounded text-sm text-beige-warm
                  hover:bg-charcoal-medium transition-colors cursor-pointer"
              >
                {opt}
              </button>
            ))}
          </div>
          <button
            onClick={cancel}
            className="mt-2 text-xs text-tan-muted hover:text-beige-warm transition-colors cursor-pointer"
          >
            Cancel
          </button>
        </div>
      );

    case 'confirm':
      return (
        <div className="mx-3 mb-3 p-3 rounded-lg border border-purple-1/40 bg-charcoal-darkest">
          <p className="text-sm text-beige-warm font-medium mb-1">{request.title}</p>
          {request.message && (
            <p className="text-xs text-tan-muted mb-3">{request.message}</p>
          )}
          <div className="flex gap-2">
            <Button text="Yes" size="sm" onClick={() => respond(request.id, { confirmed: true })} />
            <Button text="No" size="sm" variant="outline" onClick={() => respond(request.id, { confirmed: false })} />
          </div>
        </div>
      );

    case 'input':
      return (
        <div className="mx-3 mb-3 p-3 rounded-lg border border-purple-1/40 bg-charcoal-darkest">
          <p className="text-sm text-beige-warm mb-2">{request.title}</p>
          <div className="flex gap-2">
            <input
              type="text"
              autoFocus
              placeholder={request.placeholder ?? ''}
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  respond(request.id, { value: inputValue });
                  setInputValue('');
                } else if (e.key === 'Escape') {
                  cancel();
                }
              }}
              className="flex-1 px-3 py-1.5 rounded-md bg-charcoal-dark border border-charcoal-light
                text-beige-warm text-sm outline-none focus:border-tan-muted"
            />
            <Button
              text="Submit"
              size="sm"
              onClick={() => {
                respond(request.id, { value: inputValue });
                setInputValue('');
              }}
            />
          </div>
          <button
            onClick={cancel}
            className="mt-2 text-xs text-tan-muted hover:text-beige-warm transition-colors cursor-pointer"
          >
            Cancel
          </button>
        </div>
      );

    case 'editor':
      return (
        <div className="mx-3 mb-3 p-3 rounded-lg border border-purple-1/40 bg-charcoal-darkest">
          <p className="text-sm text-beige-warm mb-2">{request.title}</p>
          <textarea
            autoFocus
            defaultValue={(request as { prefill?: string }).prefill ?? ''}
            onChange={(e) => setInputValue(e.target.value)}
            className="w-full h-32 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light
              text-beige-warm font-mono text-xs outline-none focus:border-tan-muted resize-y"
          />
          <div className="flex gap-2 mt-2">
            <Button text="Submit" size="sm" onClick={() => respond(request.id, { value: inputValue })} />
            <Button text="Cancel" size="sm" variant="outline" onClick={cancel} />
          </div>
        </div>
      );

    default:
      return null;
  }
}
