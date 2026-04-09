import { useState } from 'react';
import { clsx } from 'clsx';
import type { GroupedActivityEvent } from '../../hooks/useGroupedActivity';
import { getTriggerDataLabel } from '../../types';
import { useAppStore } from '../../stores/appStore';
import {
  formatTimestamp,
  getTriggerAccent,
  DetailRows,
  SubmissionRows,
} from './ActivityCard';

interface GroupedActivityCardProps {
  group: GroupedActivityEvent;
  expanded: boolean;
  onToggleExpand: () => void;
  compact?: boolean;
}

export function GroupedActivityCard({
  group,
  expanded,
  onToggleExpand,
  compact,
}: GroupedActivityCardProps) {
  const getServiceLabel = useAppStore((state) => state.getServiceLabel);
  const [rawExpanded, setRawExpanded] = useState(false);
  const [childRawExpanded, setChildRawExpanded] = useState(false);

  const serviceName = getServiceLabel(group.trigger.serviceId);
  const triggerDataLabel = group.trigger.triggerData
    ? getTriggerDataLabel(group.trigger.triggerData)
    : 'Trigger';
  const accent = group.trigger.triggerData
    ? getTriggerAccent(group.trigger.triggerData)
    : { border: 'border-l-charcoal-light', pill: 'bg-charcoal-medium text-tan-muted' };

  return (
    <div
      className={clsx(
        'pl-3 pr-4 pt-3 pb-3 rounded-lg border border-l-4 bg-charcoal-dark transition-colors border-green-900/30',
        accent.border,
      )}
    >
      {/* Header row — full click target for expand/collapse */}
      <div
        className="flex items-center gap-2 min-w-0 cursor-pointer"
        role="button"
        onClick={onToggleExpand}
      >
        {/* Kind pill */}
        <span className="shrink-0 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wide bg-green-900/40 text-green-400">
          Trigger
        </span>

        {/* Trigger data label pill */}
        <span className={clsx('shrink-0 px-2 py-0.5 rounded text-xs font-medium', accent.pill)}>
          {triggerDataLabel}
        </span>

        {/* Status dot */}
        {group.status === 'pending' && (
          <span
            className="w-2 h-2 rounded-full bg-amber-400 animate-glow-amber shrink-0"
            aria-label="Waiting for submission"
          />
        )}
        {group.status === 'failed' && (
          <span
            className="w-2 h-2 rounded-full bg-red-400 animate-glow-red shrink-0"
            aria-label="Submission failed"
          />
        )}

        {/* Timestamp */}
        <span className="shrink-0 text-tan-muted text-xs ml-auto font-mono">
          {formatTimestamp(group.trigger.ts)}
        </span>
      </div>

      {/* Service/workflow row */}
      {!compact && (
        <div className="mt-1 text-xs text-beige-warm truncate">
          {serviceName}
          <span className="text-tan-muted"> / {group.trigger.workflowId}</span>
        </div>
      )}

      {/* Expanded content */}
      {expanded && (
        <>
          {/* Trigger detail rows */}
          {group.trigger.triggerData && (
            <DetailRows
              data={group.trigger.triggerData}
              config={group.trigger.triggerConfig}
            />
          )}

          {/* Parent raw JSON toggle */}
          <button
            type="button"
            className="mt-2 text-xs text-tan-muted hover:text-beige-warm cursor-pointer select-none"
            onClick={(e) => {
              e.stopPropagation();
              setRawExpanded((prev) => !prev);
            }}
          >
            Raw {rawExpanded ? '\u25B2' : '\u25BC'}
          </button>

          {rawExpanded && (
            <div className="mt-2 p-3 rounded bg-charcoal-darkest text-beige-light/90 font-mono text-xs leading-relaxed overflow-x-auto max-h-80 overflow-y-auto">
              <pre className="whitespace-pre-wrap">
                {group.trigger.triggerData
                  ? `// Trigger Data\n${JSON.stringify(group.trigger.triggerData, null, 2)}`
                  : '// No data'}
                {group.trigger.triggerConfig
                  ? `\n\n// Trigger Config\n${JSON.stringify(group.trigger.triggerConfig.trigger, null, 2)}`
                  : ''}
                {group.trigger.correlationId
                  ? `\n\n// Correlation ID\n${group.trigger.correlationId}`
                  : ''}
              </pre>
            </div>
          )}

          {/* Child card (submission) */}
          {group.submission && (
            <div className="ml-2 mt-2 border border-charcoal-light bg-charcoal-darkest rounded-md pl-3 pr-3 pt-3 pb-3">
              {/* Child header row */}
              <div className="flex items-center gap-2 min-w-0">
                <span
                  className={clsx(
                    'shrink-0 px-1.5 py-0.5 rounded text-[10px] font-bold uppercase tracking-wide',
                    group.submission.kind === 'submission_failed'
                      ? 'bg-red-900/40 text-red-400'
                      : 'bg-blue-900/40 text-blue-400',
                  )}
                >
                  {group.submission.kind === 'submission_failed' ? 'Failed' : 'Submit'}
                </span>

                <span className="shrink-0 text-tan-muted text-xs ml-auto font-mono">
                  {formatTimestamp(group.submission.ts)}
                </span>
              </div>

              {/* Error text — no truncate (ERR-03, ERR-04) */}
              {group.submission.error && (
                <div className="mt-1 text-xs text-red-400">
                  Error: {group.submission.error}
                </div>
              )}

              <SubmissionRows
                txHash={group.submission.txHash}
                resultPayload={group.submission.resultPayload}
                bgColor="bg-charcoal-darkest"
              />

              {/* Child raw JSON toggle */}
              <button
                type="button"
                className="mt-2 text-xs text-tan-muted hover:text-beige-warm cursor-pointer select-none"
                onClick={(e) => {
                  e.stopPropagation();
                  setChildRawExpanded((prev) => !prev);
                }}
              >
                Raw {childRawExpanded ? '\u25B2' : '\u25BC'}
              </button>

              {childRawExpanded && (
                <div className="mt-2 p-3 rounded bg-charcoal-darkest text-beige-light/90 font-mono text-xs leading-relaxed overflow-x-auto max-h-80 overflow-y-auto">
                  <pre className="whitespace-pre-wrap">
                    {group.submission.error
                      ? `// Error\n${group.submission.error}`
                      : group.submission.triggerData
                        ? `// Submission Data\n${JSON.stringify(group.submission.triggerData, null, 2)}`
                        : '// No data'}
                    {group.submission.correlationId
                      ? `\n\n// Correlation ID\n${group.submission.correlationId}`
                      : ''}
                  </pre>
                </div>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
}
