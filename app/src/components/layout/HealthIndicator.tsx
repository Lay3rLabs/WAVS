import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { getHealthStatus } from '../../tauri';
import type { HealthStatus, ChainHealthResult, ChainKey } from '../../types';
import { isChainHealthy } from '../../types';

const POLL_INTERVAL_MS = 10000;

type OverallStatus = 'loading' | 'online' | 'degraded' | 'offline';

function getOverallStatus(
  nodeOnline: boolean | null,
  healthStatus: HealthStatus | null,
): OverallStatus {
  if (nodeOnline === null) return 'loading';
  if (!nodeOnline) return 'offline';
  if (!healthStatus) return 'online';

  const allHealthy = Object.values(healthStatus.chains).every(isChainHealthy);
  return allHealthy ? 'online' : 'degraded';
}

export function HealthIndicator() {
  const navigate = useNavigate();
  const [healthStatus, setHealthStatus] = useState<HealthStatus | null>(null);
  const [nodeOnline, setNodeOnline] = useState<boolean | null>(null);

  const fetchHealth = useCallback(async () => {
    try {
      const status = await getHealthStatus();
      setHealthStatus(status);
      setNodeOnline(true);
    } catch {
      setNodeOnline(false);
      setHealthStatus(null);
    }
  }, []);

  useEffect(() => {
    fetchHealth();
    const interval = setInterval(fetchHealth, POLL_INTERVAL_MS);

    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') fetchHealth();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);

    return () => {
      clearInterval(interval);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, [fetchHealth]);

  const status = getOverallStatus(nodeOnline, healthStatus);

  const dotClass = {
    loading: 'bg-yellow-500 animate-glow-yellow',
    online:  'bg-green-500  animate-glow-green',
    degraded: 'bg-amber-500 animate-glow-amber',
    offline: 'bg-red-500    animate-glow-red',
  }[status];

  const statusLabel = {
    loading:  'Checking',
    online:   'Online',
    degraded: 'Degraded',
    offline:  'Offline',
  }[status];

  const labelClass = {
    loading:  'text-yellow-400',
    online:   'text-green-400',
    degraded: 'text-amber-400',
    offline:  'text-red-400',
  }[status];

  const chainEntries = healthStatus
    ? (Object.entries(healthStatus.chains) as [ChainKey, ChainHealthResult][])
    : [];
  const healthyCount = chainEntries.filter(([, r]) => isChainHealthy(r)).length;
  const unhealthyCount = chainEntries.length - healthyCount;

  const tooltipLines: string[] =
    status === 'loading'
      ? ['Checking health…']
      : status === 'offline'
        ? ['Node: Offline']
        : [
            'Node: Online',
            ...(chainEntries.length > 0
              ? [
                  `${healthyCount} chain${healthyCount !== 1 ? 's' : ''} healthy` +
                    (unhealthyCount > 0
                      ? `, ${unhealthyCount} unhealthy`
                      : ''),
                ]
              : ['No chains configured']),
          ];

  return (
    <div className="relative group">
      <button
        onClick={() => navigate('/health')}
        className="flex items-center gap-2 px-2 py-1.5 rounded-full hover:bg-charcoal-medium transition-colors cursor-pointer"
        aria-label="Health status"
      >
        <div className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${dotClass}`} />
        <span className={`text-xs font-medium tracking-wide ${labelClass}`}>
          {statusLabel}
        </span>
      </button>

      {/* Tooltip */}
      <div className="absolute left-1/2 -translate-x-1/2 top-full mt-2 px-3 py-2 rounded bg-charcoal-darkest border border-charcoal-light text-xs text-beige-warm whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50 flex flex-col gap-0.5">
        {tooltipLines.map((line) => (
          <span key={line}>{line}</span>
        ))}
        {/* Arrow */}
        <div className="absolute left-1/2 -translate-x-1/2 -top-1 w-2 h-2 rotate-45 bg-charcoal-darkest border-l border-t border-charcoal-light" />
      </div>
    </div>
  );
}
