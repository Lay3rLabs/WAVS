import { useState, useEffect, useCallback } from 'react';
import { Button, AddressDisplay } from '../../components/atoms';
import { getP2pStatus, getServiceSigner } from '../../tauri';
import type { P2pStatus, Service, SignerResponse } from '../../types';
import { getErrorMessage } from '../../types';
import { useAppStore } from '../../stores/appStore';
import { getPublicClient } from '../../hooks/useViemClient';
import { POAStakeRegistryABI } from '../../contracts/POAStakeRegistry';
import type { Address } from 'viem';

const REFRESH_INTERVAL_MS = 15000;

type RegistrationStatus = 'registered' | 'unregistered' | 'unknown' | 'na';

interface ServiceSignerInfo {
  signer: SignerResponse | null;
  registrationStatus: RegistrationStatus;
}

export function P2pPage() {
  const [p2pStatus, setP2pStatus] = useState<P2pStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [signerInfo, setSignerInfo] = useState<Map<string, ServiceSignerInfo>>(new Map());
  const services = useAppStore((state) => state.services);

  const fetchSignerInfo = useCallback(async (serviceHashes: string[]) => {
    const newInfo = new Map<string, ServiceSignerInfo>();

    for (const hash of serviceHashes) {
      const service = services.get(hash);
      if (!service) {
        newInfo.set(hash, { signer: null, registrationStatus: 'unknown' });
        continue;
      }

      // Fetch signer
      let signer: SignerResponse | null = null;
      try {
        signer = await getServiceSigner(service.manager);
      } catch {
        // Signer not available
      }

      // Check on-chain registration
      let registrationStatus: RegistrationStatus = 'unknown';
      if ('evm' in service.manager) {
        try {
          const chainKey = service.manager.evm.chain;

          // Find a matching saved registry for this service's chain
          const settings = useAppStore.getState().settings;
          const savedRegistry = settings.saved_registries.find(
            (r) => r.chain_key === chainKey,
          );

          if (savedRegistry && signer) {
            const publicClient = getPublicClient(savedRegistry.rpc_url, savedRegistry.chain_id);
            const operatorAddress =
              'secp256k1' in signer
                ? (signer.secp256k1.evm_address as Address)
                : null;

            if (operatorAddress) {
              const isRegistered = await publicClient.readContract({
                address: savedRegistry.address as Address,
                abi: POAStakeRegistryABI,
                functionName: 'operatorRegistered',
                args: [operatorAddress],
              });
              registrationStatus = isRegistered ? 'registered' : 'unregistered';
            } else {
              // BLS signer -- registration check requires EVM operator address
              // BLS registration is Phase 11 scope
              registrationStatus = 'unknown';
            }
          } else if (!savedRegistry) {
            registrationStatus = 'unknown';
          }
        } catch {
          registrationStatus = 'unknown';
        }
      } else {
        // Cosmos service -- registration checks are EVM-only
        registrationStatus = 'na';
      }

      newInfo.set(hash, { signer, registrationStatus });
    }

    setSignerInfo(newInfo);
  }, [services]);

  const handleRefresh = useCallback(async () => {
    setIsRefreshing(true);
    try {
      const status = await getP2pStatus();
      setP2pStatus(status);
      setError(null);
      setLastRefresh(new Date());
      if (status.subscribed_services.length > 0) {
        await fetchSignerInfo(status.subscribed_services);
      }
    } catch (err) {
      setError(getErrorMessage(err));
      setP2pStatus(null);
    } finally {
      setIsRefreshing(false);
    }
  }, [fetchSignerInfo]);

  useEffect(() => {
    const init = async () => {
      try {
        const status = await getP2pStatus();
        setP2pStatus(status);
        setError(null);
        setLastRefresh(new Date());
        // Fetch signer info for subscribed services (one-time on mount)
        if (status.subscribed_services.length > 0) {
          fetchSignerInfo(status.subscribed_services);
        }
      } catch (err) {
        setError(getErrorMessage(err));
      }
    };
    init();

    const interval = setInterval(async () => {
      setIsRefreshing(true);
      try {
        const status = await getP2pStatus();
        setP2pStatus(status);
        setError(null);
        setLastRefresh(new Date());
      } catch (err) {
        setError(getErrorMessage(err));
      } finally {
        setIsRefreshing(false);
      }
    }, REFRESH_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [fetchSignerInfo]);

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
            onClick={handleRefresh}
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
              <ServicesCard status={p2pStatus} services={services} signerInfo={signerInfo} />
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
            <AddressDisplay address={status.local_peer_id} full alwaysShowCopy />
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
              <AddressDisplay address={peerId} full alwaysShowCopy />
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
  signerInfo,
}: {
  status: P2pStatus;
  services: Map<string, Service>;
  signerInfo: Map<string, ServiceSignerInfo>;
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
          {status.subscribed_services.map((hash) => {
            const service = services.get(hash);
            const info = signerInfo.get(hash);
            return (
              <ServiceOperatorRow
                key={hash}
                serviceHash={hash}
                serviceName={service?.name ?? hash.slice(0, 12) + '...'}
                signer={info?.signer ?? null}
                registrationStatus={info?.registrationStatus ?? 'unknown'}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

function ServiceOperatorRow({
  serviceHash: _serviceHash,
  serviceName,
  signer,
  registrationStatus,
}: {
  serviceHash: string;
  serviceName: string;
  signer: SignerResponse | null;
  registrationStatus: RegistrationStatus;
}) {
  const isSecp = signer && 'secp256k1' in signer;
  const isBls = signer && 'bls12381' in signer;
  const keyDisplay = isSecp
    ? signer.secp256k1.evm_address
    : isBls
      ? signer.bls12381.g1_pubkey_hex
      : null;
  const algorithmLabel = isSecp ? 'ECDSA' : isBls ? 'BLS' : null;

  return (
    <div className="p-4 rounded bg-charcoal-dark border border-charcoal-light">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-beige-light font-medium">{serviceName}</span>
        {algorithmLabel && (
          <span className="inline-flex items-center px-2 py-1 rounded bg-charcoal-light text-xs font-semibold text-beige-warm">
            {algorithmLabel}
          </span>
        )}
        <RegistrationBadge status={registrationStatus} />
      </div>
      {keyDisplay && (
        <div className="flex items-center gap-2">
          <span className="text-tan-muted text-xs font-semibold">Operator Key</span>
          <AddressDisplay address={keyDisplay} full alwaysShowCopy />
        </div>
      )}
    </div>
  );
}

function RegistrationBadge({ status }: { status: RegistrationStatus }) {
  const styles: Record<RegistrationStatus, { bg: string; text: string; label: string }> = {
    registered: {
      bg: 'bg-success-900/30',
      text: 'text-success-500',
      label: 'Registered',
    },
    unregistered: {
      bg: 'bg-charcoal-light',
      text: 'text-tan-muted',
      label: 'Unregistered',
    },
    unknown: {
      bg: 'bg-charcoal-light',
      text: 'text-tan-muted',
      label: 'Unknown',
    },
    na: {
      bg: 'bg-charcoal-light',
      text: 'text-tan-muted',
      label: 'N/A',
    },
  };

  const style = styles[status];
  return (
    <span
      className={`inline-flex items-center px-2 py-1 rounded text-xs font-semibold ${style.bg} ${style.text}`}
      title={
        status === 'na'
          ? 'Registration checks are only available for EVM services'
          : undefined
      }
    >
      {style.label}
    </span>
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
