import { useMemo } from 'react';
import type { ActivityItem } from '../types';

export interface GroupedActivityEvent {
  trigger: ActivityItem;
  submission?: ActivityItem;
  status: 'pending' | 'complete' | 'failed';
  groupKey: string; // correlationId if present, else String(trigger.id)
}

export const STATUS_TABS = ['all', 'pending', 'failed', 'complete'] as const;
export type StatusFilter = typeof STATUS_TABS[number];

export function useGroupedActivity(sourceList: ActivityItem[]): {
  groups: GroupedActivityEvent[];
  orphans: ActivityItem[];
} {
  return useMemo(() => {
    const byCorrelation = new Map<string, GroupedActivityEvent>();
    const orphans: ActivityItem[] = [];

    for (const item of sourceList) {
      if (item.kind === 'trigger') {
        const key = item.correlationId ?? String(item.id);
        // First-write-wins: defensive against duplicate correlationId
        if (!byCorrelation.has(key)) {
          byCorrelation.set(key, {
            trigger: item,
            submission: undefined,
            status: 'pending',
            groupKey: key,
          });
        }
      } else if (item.kind === 'submission' || item.kind === 'submission_failed') {
        if (item.correlationId !== undefined && byCorrelation.has(item.correlationId)) {
          const group = byCorrelation.get(item.correlationId)!;
          group.submission = item;
          group.status = item.kind === 'submission_failed' ? 'failed' : 'complete';
        } else {
          orphans.push(item);
        }
      } else if (item.kind === 'execution_complete') {
        // submit:"none" services don't carry a correlation_id, so match on
        // serviceId+workflowId to the oldest still-pending group (FIFO),
        // which is the trigger this execution responded to.
        let matched: GroupedActivityEvent | undefined;
        for (const group of byCorrelation.values()) {
          if (
            group.status === 'pending' &&
            group.trigger.serviceId === item.serviceId &&
            group.trigger.workflowId === item.workflowId &&
            (matched === undefined || group.trigger.ts < matched.trigger.ts)
          ) {
            matched = group;
          }
        }
        if (matched !== undefined) {
          matched.submission = item;
          matched.status = 'complete';
        } else {
          orphans.push(item);
        }
      }
    }

    return { groups: Array.from(byCorrelation.values()), orphans };
  }, [sourceList]);
}
