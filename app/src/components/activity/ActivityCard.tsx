import { clsx } from 'clsx';
import type { UnifiedActivity, ActivityStatus, TriggerData, TriggerConfig } from '../../types';
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

function formatBlockNumber(n: number): string {
  return `#${n.toLocaleString()}`;
}

function getTriggerAccent(data: TriggerData): { border: string; pill: string } {
  if ('EvmContractEvent' in data) {
    return { border: 'border-l-indigo-500', pill: 'bg-indigo-900/50 text-indigo-300' };
  }
  if ('CosmosContractEvent' in data) {
    return { border: 'border-l-violet-500', pill: 'bg-violet-900/50 text-violet-300' };
  }
  if ('BlockInterval' in data) {
    return { border: 'border-l-cyan-600', pill: 'bg-cyan-900/50 text-cyan-300' };
  }
  if ('Cron' in data) {
    return { border: 'border-l-amber-500', pill: 'bg-amber-900/50 text-amber-300' };
  }
  if ('AtProtoEvent' in data) {
    return { border: 'border-l-sky-500', pill: 'bg-sky-900/50 text-sky-300' };
  }
  if ('HypercoreAppend' in data) {
    return { border: 'border-l-emerald-500', pill: 'bg-emerald-900/50 text-emerald-300' };
  }
  return { border: 'border-l-charcoal-light', pill: 'bg-charcoal-medium text-tan-muted' };
}

function DetailRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex gap-3 text-xs">
      <span className="text-tan-muted w-20 shrink-0">{label}</span>
      <span className="text-beige-warm font-mono break-all">{value}</span>
    </div>
  );
}

function DetailRows({ data, config }: { data: TriggerData; config?: TriggerConfig }) {
  if ('EvmContractEvent' in data) {
    const d = data.EvmContractEvent;
    return (
      <div className="flex flex-col gap-1 mt-2">
        <DetailRow label="chain" value={d.chain} />
        <DetailRow label="block" value={formatBlockNumber(d.block_number)} />
        <DetailRow label="tx" value={d.tx_hash} />
        <DetailRow label="contract" value={d.contract_address} />
        <DetailRow label="log_index" value={String(d.log_index)} />
      </div>
    );
  }

  if ('CosmosContractEvent' in data) {
    const d = data.CosmosContractEvent;
    return (
      <div className="flex flex-col gap-1 mt-2">
        <DetailRow label="chain" value={d.chain} />
        <DetailRow label="block" value={formatBlockNumber(d.block_height)} />
        <DetailRow label="contract" value={d.contract_address} />
        <DetailRow label="event_idx" value={String(d.event_index)} />
      </div>
    );
  }

  if ('BlockInterval' in data) {
    const d = data.BlockInterval;
    return (
      <div className="flex flex-col gap-1 mt-2">
        <DetailRow label="chain" value={d.chain} />
        <DetailRow label="block" value={formatBlockNumber(d.block_height)} />
      </div>
    );
  }

  if ('Cron' in data) {
    const ts = data.Cron.trigger_time;
    const ms = ts > 1e12 ? ts : ts * 1000;
    const fired = new Date(ms);
    const firedStr = isNaN(fired.getTime())
      ? String(ts)
      : fired.toLocaleString('en-US', {
          weekday: 'short',
          year: 'numeric',
          month: 'short',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit',
          second: '2-digit',
        });
    const schedule =
      config?.trigger &&
      typeof config.trigger !== 'string' &&
      'cron' in config.trigger
        ? config.trigger.cron.schedule
        : null;
    return (
      <div className="flex flex-col gap-1 mt-2">
        <DetailRow label="fired" value={firedStr} />
        {schedule && <DetailRow label="schedule" value={schedule} />}
      </div>
    );
  }

  if ('AtProtoEvent' in data) {
    const d = data.AtProtoEvent;
    const actionColor =
      d.action === 'create'
        ? 'text-green-400'
        : d.action === 'update'
          ? 'text-amber-400'
          : 'text-red-400';
    return (
      <div className="flex flex-col gap-1 mt-2">
        <DetailRow
          label="action"
          value={
            <span className={clsx('font-mono uppercase font-bold', actionColor)}>
              {d.action}
            </span>
          }
        />
        <DetailRow label="collection" value={d.collection} />
        <DetailRow
          label="repo"
          value={d.repo.length > 20 ? d.repo.slice(0, 20) + '...' : d.repo}
        />
        <DetailRow label="rkey" value={d.rkey} />
      </div>
    );
  }

  if ('HypercoreAppend' in data) {
    const d = data.HypercoreAppend;
    return (
      <div className="flex flex-col gap-1 mt-2">
        <DetailRow
          label="feed"
          value={d.feed_key.length > 20 ? d.feed_key.slice(0, 20) + '...' : d.feed_key}
        />
        <DetailRow label="index" value={String(d.index)} />
        <DetailRow label="data" value={`${d.data.length} bytes`} />
      </div>
    );
  }

  if ('Raw' in data) {
    return (
      <div className="flex flex-col gap-1 mt-2">
        <DetailRow label="size" value={`${data.Raw.length} bytes`} />
      </div>
    );
  }

  return null;
}

function StatusBadge({ status }: { status: ActivityStatus }) {
  const styles = {
    pending: 'bg-amber-900/40 text-amber-400',
    executed: 'bg-blue-900/40 text-blue-400',
    confirmed: 'bg-green-900/40 text-green-400',
    error: 'bg-red-900/40 text-red-400',
  };
  const labels = {
    pending: 'PENDING',
    executed: 'EXECUTED',
    confirmed: 'CONFIRMED',
    error: 'ERROR',
  };
  return (
    <span className={clsx('shrink-0 px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wide', styles[status])}>
      {labels[status]}
    </span>
  );
}

function truncateTxHash(hash: string): string {
  if (hash.length <= 18) return hash;
  return `${hash.slice(0, 10)}...${hash.slice(-8)}`;
}

function SubmissionSection({ item }: { item: UnifiedActivity }) {
  if (item.status === 'pending' || item.status === 'executed') return null;

  if (item.status === 'confirmed') {
    return (
      <div className="mt-2 px-3 py-2 rounded bg-charcoal-darkest border border-charcoal-light/30">
        <div className="flex items-center gap-2 text-xs">
          <span className="text-tan-muted">Submitted</span>
          {item.submissionTs && (
            <span className="text-tan-muted font-mono">{formatTimestamp(item.submissionTs)}</span>
          )}
          {item.txHash ? (
            <span className="font-mono text-xs text-primary-500">{truncateTxHash(item.txHash)}</span>
          ) : (
            <span className="text-tan-muted font-mono">(no hash)</span>
          )}
        </div>
      </div>
    );
  }

  // status === 'error'
  return (
    <div className="mt-2 px-3 py-2 rounded bg-charcoal-darkest border border-red-900/30">
      <div className="flex items-center gap-2 text-xs">
        <span className="text-red-400 font-semibold">Error</span>
        {item.submissionTs && (
          <span className="text-tan-muted font-mono">{formatTimestamp(item.submissionTs)}</span>
        )}
      </div>
      {item.errorMessage && (
        <div className="text-red-400 text-xs mt-1 break-words max-h-20 overflow-y-auto">
          {item.errorMessage}
        </div>
      )}
    </div>
  );
}

interface ActivityCardProps {
  item: UnifiedActivity;
  expanded: boolean;
  onToggleExpand: () => void;
  compact?: boolean;
}

export function ActivityCard({ item, expanded, onToggleExpand, compact }: ActivityCardProps) {
  const getServiceLabel = useAppStore((state) => state.getServiceLabel);

  const serviceName = getServiceLabel(item.serviceId);
  const triggerLabel = getTriggerDataLabel(item.triggerData);
  const accent = getTriggerAccent(item.triggerData);

  const statusBorder = {
    pending: 'border-charcoal-light/30',
    executed: 'border-blue-900/30',
    confirmed: 'border-green-900/30',
    error: 'border-red-900/30',
  }[item.status];

  return (
    <div
      className={clsx(
        'pl-3 pr-4 pt-3 pb-3 rounded-lg border border-l-4 bg-charcoal-dark transition-colors',
        statusBorder,
        accent.border,
      )}
    >
      {/* Header row */}
      <div className="flex items-center gap-2 min-w-0">
        <StatusBadge status={item.status} />

        <span className={clsx('shrink-0 px-2 py-0.5 rounded text-xs font-medium', accent.pill)}>
          {triggerLabel}
        </span>

        <span className="shrink-0 text-tan-muted text-xs ml-auto font-mono">
          {formatTimestamp(item.triggerTs)}
        </span>
      </div>

      {!compact && (
        <div className="mt-1 text-xs text-beige-warm truncate">
          {serviceName}
          <span className="text-tan-muted"> / {item.workflowId}</span>
        </div>
      )}

      <DetailRows data={item.triggerData} config={item.triggerConfig} />

      <SubmissionSection item={item} />

      <button
        type="button"
        className="mt-2 text-xs text-tan-muted hover:text-beige-warm cursor-pointer select-none"
        onClick={onToggleExpand}
      >
        Raw {expanded ? '\u25B2' : '\u25BC'}
      </button>

      {expanded && (
        <div className="mt-2 p-3 rounded bg-charcoal-darkest text-beige-light/90 font-mono text-xs leading-relaxed overflow-x-auto max-h-80 overflow-y-auto">
          <pre className="whitespace-pre-wrap">
            {`// Trigger Data\n${JSON.stringify(item.triggerData, null, 2)}`}
            {item.triggerConfig
              ? `\n\n// Trigger Config\n${JSON.stringify(item.triggerConfig.trigger, null, 2)}`
              : ''}
            {item.txHash ? `\n\n// Tx Hash\n${item.txHash}` : ''}
            {item.errorMessage ? `\n\n// Error\n${item.errorMessage}` : ''}
          </pre>
        </div>
      )}
    </div>
  );
}
