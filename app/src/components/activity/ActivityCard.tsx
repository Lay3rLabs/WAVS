import { clsx } from 'clsx';
import type { ActivityItem, TriggerData } from '../../types';
import { getTriggerDataLabel } from '../../types';
import { useAppStore } from '../../stores/appStore';

function formatTimestamp(ts: number): string {
  const date = new Date(ts);
  const hours = date.getHours().toString().padStart(2, '0');
  const mins = date.getMinutes().toString().padStart(2, '0');
  const secs = date.getSeconds().toString().padStart(2, '0');
  const millis = date.getMilliseconds().toString().padStart(3, '0');
  return `${hours}:${mins}:${secs}.${millis}`;
}

export function getActivitySummary(data: TriggerData): string {
  if ('BlockInterval' in data) {
    return `Block #${data.BlockInterval.block_height} on ${data.BlockInterval.chain}`;
  }
  if ('EvmContractEvent' in data) {
    const d = data.EvmContractEvent;
    return `Block #${d.block_number} tx:${d.tx_hash.slice(0, 10)}... on ${d.chain}`;
  }
  if ('CosmosContractEvent' in data) {
    const d = data.CosmosContractEvent;
    return `Block #${d.block_height} on ${d.chain}`;
  }
  if ('Cron' in data) {
    const ts = data.Cron.trigger_time;
    if (!ts) return 'Cron triggered';
    const ms = ts > 1e12 ? ts : ts * 1000;
    const date = new Date(ms);
    if (isNaN(date.getTime())) return 'Cron triggered';
    return `Cron fired at ${date.toLocaleTimeString()}`;
  }
  if ('AtProtoEvent' in data) {
    const d = data.AtProtoEvent;
    return `${d.action} ${d.collection} seq:${d.sequence}`;
  }
  if ('HypercoreAppend' in data) {
    const d = data.HypercoreAppend;
    return `Feed ${d.feed_key.slice(0, 12)}... index:${d.index}`;
  }
  if ('Raw' in data) {
    return `${data.Raw.length} bytes`;
  }
  return '';
}

interface ActivityCardProps {
  item: ActivityItem;
  expanded: boolean;
  onToggleExpand: () => void;
}

export function ActivityCard({ item, expanded, onToggleExpand }: ActivityCardProps) {
  const getServiceLabel = useAppStore((state) => state.getServiceLabel);

  const serviceName = getServiceLabel(item.serviceId);
  const triggerLabel = getTriggerDataLabel(item.triggerData);
  const summary = getActivitySummary(item.triggerData);
  const isTrigger = item.kind === 'trigger';

  return (
    <div
      className={clsx(
        'p-4 rounded-lg border bg-charcoal-dark transition-colors',
        isTrigger ? 'border-green-900/50' : 'border-blue-900/50'
      )}
    >
      {/* Header row */}
      <div className="flex items-center gap-2.5 min-w-0">
        <span
          className={clsx(
            'shrink-0 px-2 py-0.5 rounded text-[11px] font-bold uppercase tracking-wide',
            isTrigger
              ? 'bg-green-800/60 text-green-300'
              : 'bg-blue-800/60 text-blue-300'
          )}
        >
          {isTrigger ? 'Trigger' : 'Submit'}
        </span>

        <span className="shrink-0 text-tan-muted text-xs px-1.5 py-0.5 rounded bg-charcoal-darkest/60">
          {triggerLabel}
        </span>

        <span className="text-beige-warm text-sm truncate">
          {serviceName}
          <span className="text-tan-muted"> / {item.workflowId}</span>
        </span>

        <span className="shrink-0 text-tan-muted text-xs ml-auto font-mono">
          {formatTimestamp(item.ts)}
        </span>
      </div>

      {summary && (
        <div className="mt-2 text-sm text-beige-light/80 font-mono pl-0.5">
          {summary}
        </div>
      )}

      <button
        type="button"
        className="mt-2 text-xs text-tan-muted hover:text-beige-warm cursor-pointer select-none"
        onClick={onToggleExpand}
      >
        {expanded ? 'Hide details' : 'Details'} {expanded ? '▲' : '▼'}
      </button>

      {expanded && (
        <div className="mt-2 p-3 rounded bg-charcoal-darkest text-beige-light/90 font-mono text-xs leading-relaxed overflow-x-auto max-h-80 overflow-y-auto">
          <pre className="whitespace-pre-wrap">
            {JSON.stringify(item, null, 2)}
          </pre>
        </div>
      )}
    </div>
  );
}
