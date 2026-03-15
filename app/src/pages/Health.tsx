import { useState, useEffect, useCallback } from 'react';
import { Button } from '../components/atoms';
import { getHealthStatus, getMcpStatus } from '../tauri';
import type { HealthStatus, ChainHealthResult, ChainKey, McpStatus } from '../types';
import { isChainHealthy, getChainError, getErrorMessage } from '../types';

const REFRESH_INTERVAL_MS = 30000;

type NodeStatus = 'online' | 'offline' | 'loading';

function formatTimestamp(ts: number): string {
  const date = new Date(ts * 1000);
  return date.toLocaleTimeString();
}

export function Health() {
  const [healthStatus, setHealthStatus] = useState<HealthStatus | null>(null);
  const [nodeStatus, setNodeStatus] = useState<NodeStatus>('loading');
  const [error, setError] = useState<string | null>(null);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [mcpStatus, setMcpStatus] = useState<McpStatus | null>(null);

  const fetchHealth = useCallback(async () => {
    setIsRefreshing(true);
    try {
      const status = await getHealthStatus();
      setHealthStatus(status);
      setNodeStatus('online');
      setError(null);
      setLastRefresh(new Date());
    } catch (err) {
      setNodeStatus('offline');
      setError(getErrorMessage(err));
      setHealthStatus(null);
    } finally {
      setIsRefreshing(false);
    }
    try {
      setMcpStatus(await getMcpStatus());
    } catch {
      // not fatal — leave previous status in place
    }
  }, []);

  useEffect(() => {
    fetchHealth();

    const interval = setInterval(fetchHealth, REFRESH_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [fetchHealth]);

  const chainEntries = healthStatus
    ? Object.entries(healthStatus.chains) as [ChainKey, ChainHealthResult][]
    : [];

  const healthyCount = chainEntries.filter(([, result]) => isChainHealthy(result)).length;
  const unhealthyCount = chainEntries.length - healthyCount;

  return (
    <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {/* Header with refresh */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-beige-light">Health Status</h1>
        <div className="flex items-center gap-4">
          {lastRefresh && (
            <span className="text-sm text-tan-muted">
              Last updated: {lastRefresh.toLocaleTimeString()}
            </span>
          )}
          <Button
            text={isRefreshing ? 'Refreshing...' : 'Refresh'}
            onClick={fetchHealth}
            disabled={isRefreshing}
          />
        </div>
      </div>

      {/* WAVS Node Status */}
      <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h2 className="text-xl font-semibold text-beige-light mb-4">WAVS Node</h2>
        <div className="flex items-center gap-3">
          <StatusIndicator status={nodeStatus} />
          <span className="text-beige-warm">
            {nodeStatus === 'loading' && 'Checking...'}
            {nodeStatus === 'online' && 'Online'}
            {nodeStatus === 'offline' && 'Offline'}
          </span>
        </div>
        {error && (
          <div className="mt-3 p-3 rounded bg-red-900/20 border border-red-800 text-red-3 text-sm">
            {error}
          </div>
        )}
      </div>

      {/* MCP Server */}
      <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h2 className="text-xl font-semibold text-beige-light mb-4">MCP Server</h2>
        <div className="flex items-center gap-3">
          <div className={`w-3 h-3 rounded-full ${
            mcpStatus === null
              ? 'bg-yellow-500 animate-pulse'
              : mcpStatus.running
                ? 'bg-green-500'
                : 'bg-charcoal-light'
          }`} />
          <span className="text-beige-warm">
            {mcpStatus === null && 'Checking...'}
            {mcpStatus !== null && mcpStatus.running && `Running (pid ${mcpStatus.pid})`}
            {mcpStatus !== null && !mcpStatus.running && 'Not running'}
          </span>
        </div>
      </div>

      {/* Chain RPCs Summary */}
      {nodeStatus === 'online' && (
        <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
          <h2 className="text-xl font-semibold text-beige-light mb-4">Chain RPCs</h2>

          {chainEntries.length === 0 ? (
            <div className="text-tan-muted italic">No chains configured</div>
          ) : (
            <>
              {/* Summary */}
              <div className="flex gap-6 mb-4">
                <div className="flex items-center gap-2">
                  <div className="w-3 h-3 rounded-full bg-green-500" />
                  <span className="text-beige-warm">{healthyCount} healthy</span>
                </div>
                {unhealthyCount > 0 && (
                  <div className="flex items-center gap-2">
                    <div className="w-3 h-3 rounded-full bg-red-500" />
                    <span className="text-beige-warm">{unhealthyCount} unhealthy</span>
                  </div>
                )}
              </div>

              {/* Chain List */}
              <div className="flex flex-col gap-2">
                {chainEntries.map(([chainKey, result]) => (
                  <ChainHealthItem key={chainKey} chainKey={chainKey} result={result} />
                ))}
              </div>

              {/* Timestamp */}
              {healthStatus && (
                <div className="mt-4 text-sm text-tan-muted">
                  Health check timestamp: {formatTimestamp(healthStatus.timestamp)}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}

function StatusIndicator({ status }: { status: NodeStatus }) {
  const colorClass = {
    loading: 'bg-yellow-500 animate-pulse',
    online: 'bg-green-500',
    offline: 'bg-red-500',
  }[status];

  return <div className={`w-3 h-3 rounded-full ${colorClass}`} />;
}

function ChainHealthItem({
  chainKey,
  result,
}: {
  chainKey: ChainKey;
  result: ChainHealthResult;
}) {
  const healthy = isChainHealthy(result);
  const errorMsg = getChainError(result);

  return (
    <div className="p-3 rounded bg-charcoal-dark border border-charcoal-light">
      <div className="flex items-center gap-3">
        <div
          className={`w-2 h-2 rounded-full ${healthy ? 'bg-green-500' : 'bg-red-500'}`}
        />
        <span className="font-mono text-beige-warm">{chainKey}</span>
        <span className={`text-sm ${healthy ? 'text-green-400' : 'text-red-400'}`}>
          {healthy ? 'Healthy' : 'Unhealthy'}
        </span>
      </div>
      {errorMsg && (
        <div className="mt-2 ml-5 text-sm text-red-3 font-mono">{errorMsg}</div>
      )}
    </div>
  );
}
