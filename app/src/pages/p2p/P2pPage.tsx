import { useState, useEffect, useCallback } from 'react';
import { Button, AddressDisplay } from '../../components/atoms';
import { getP2pStatus } from '../../tauri';
import type { P2pStatus, Service } from '../../types';
import { getErrorMessage } from '../../types';
import { useAppStore } from '../../stores/appStore';

const REFRESH_INTERVAL_MS = 15000;

export function P2pPage() {
  const [p2pStatus, setP2pStatus] = useState<P2pStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const services = useAppStore((state) => state.services);

  const fetchP2pStatus = useCallback(async () => {
    setIsRefreshing(true);
    try {
      const status = await getP2pStatus();
      setP2pStatus(status);
      setError(null);
      setLastRefresh(new Date());
    } catch (err) {
      setError(getErrorMessage(err));
      setP2pStatus(null);
    } finally {
      setIsRefreshing(false);
    }
  }, []);

  useEffect(() => {
    fetchP2pStatus();

    const interval = setInterval(fetchP2pStatus, REFRESH_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [fetchP2pStatus]);

  return (
    <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {/* Header with refresh */}
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-beige-light">P2P Network</h1>
        <div className="flex items-center gap-4">
          {lastRefresh && (
            <span className="text-sm text-tan-muted">
              Last updated: {lastRefresh.toLocaleTimeString()}
            </span>
          )}
          <Button
            text={isRefreshing ? 'Refreshing...' : 'Refresh Status'}
            onClick={fetchP2pStatus}
            disabled={isRefreshing}
          />
        </div>
      </div>

      {/* Error state (WAVS not running) */}
      {error && !p2pStatus && <ErrorState message={error} />}

      {/* Content */}
      {p2pStatus && (
        <>
          {!p2pStatus.enabled ? (
            <DisabledState />
          ) : (
            <>
              <IdentityCard status={p2pStatus} />
              <PeersCard status={p2pStatus} />
              <ServicesCard status={p2pStatus} services={services} />
              <QuorumPlaceholder />
            </>
          )}
        </>
      )}
    </div>
  );
}

function IdentityCard({ status }: { status: P2pStatus }) {
  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-xl font-semibold text-beige-light mb-4">Node Identity</h2>
      <div className="grid grid-cols-2 gap-4">
        {/* Peer ID */}
        <div className="flex flex-col gap-1">
          <span className="text-tan-muted text-xs font-semibold">Peer ID (Ed25519)</span>
          {status.local_peer_id ? (
            <AddressDisplay address={status.local_peer_id} />
          ) : (
            <span className="text-tan-muted italic">Not available</span>
          )}
        </div>

        {/* Discovery Mode */}
        <div className="flex flex-col gap-1">
          <span className="text-tan-muted text-xs font-semibold">Discovery Mode</span>
          <span className="text-beige-warm capitalize">
            {status.discovery_mode || 'unknown'}
          </span>
        </div>

        {/* Listen Addresses */}
        <div className="col-span-2 flex flex-col gap-1">
          <span className="text-tan-muted text-xs font-semibold">Listen Addresses</span>
          {status.listen_addresses.length > 0 ? (
            <div className="flex flex-col gap-1">
              {status.listen_addresses.map((addr) => (
                <span key={addr} className="font-mono text-sm text-beige-warm">
                  {addr}
                </span>
              ))}
            </div>
          ) : (
            <span className="text-tan-muted italic">None</span>
          )}
        </div>
      </div>
    </div>
  );
}

function PeersCard({ status }: { status: P2pStatus }) {
  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold text-beige-light">Connected Peers</h2>
        <span className="inline-flex items-center px-2 py-1 rounded-full bg-charcoal-light text-xs font-semibold text-beige-warm">
          {status.connected_peers} connected
        </span>
      </div>
      {status.peer_ids.length === 0 ? (
        <div className="text-tan-muted italic">No peers connected</div>
      ) : (
        <div className="flex flex-col gap-2">
          {status.peer_ids.map((peerId) => (
            <div
              key={peerId}
              className="p-3 rounded bg-charcoal-dark border border-charcoal-light flex items-center gap-3"
            >
              <div className="w-2 h-2 rounded-full bg-success-600" />
              <AddressDisplay address={peerId} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ServicesCard({
  status,
  services,
}: {
  status: P2pStatus;
  services: Map<string, Service>;
}) {
  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-xl font-semibold text-beige-light mb-4">Subscribed Services</h2>
      {status.subscribed_services.length === 0 ? (
        <div className="text-tan-muted italic">
          No services subscribed to P2P topics. Deploy a service to begin participating in the
          network.
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {status.subscribed_services.map((serviceHash) => (
            <div
              key={serviceHash}
              className="p-3 rounded bg-charcoal-dark border border-charcoal-light"
            >
              <span className="text-beige-warm font-medium">
                {services.get(serviceHash)?.name ?? serviceHash.slice(0, 12) + '...'}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function QuorumPlaceholder() {
  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-xl font-semibold text-beige-light mb-4">Quorum Accumulation</h2>
      <p className="text-tan-muted italic">
        Quorum data not available — requires /aggregator/status endpoint
      </p>
    </div>
  );
}

function DisabledState() {
  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-xl font-semibold text-beige-light mb-4">P2P Networking Disabled</h2>
      <p className="text-tan-muted">
        This node is running in single-operator mode. P2P features require enabling local or remote
        discovery in the WAVS configuration.
      </p>
    </div>
  );
}

function ErrorState({ message }: { message: string }) {
  return (
    <div className="p-6 rounded-lg bg-red-900/20 border border-red-800">
      <h2 className="text-xl font-semibold text-beige-light mb-4">WAVS Node Not Running</h2>
      <p className="text-tan-muted">Start the WAVS node to view P2P network status.</p>
      <p className="mt-2 text-sm text-red-3">{message}</p>
    </div>
  );
}
