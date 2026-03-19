import { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { AddressDisplay, Button, Modal, Toast, Tabs } from '../../components/atoms';
import { OperatorList } from '../../components/poa';
import { OwnerActionsMenu } from '../../components/poa/OwnerActionsMenu';
import { WorkflowViewer } from '../../components/service/WorkflowViewer';
import { ServiceActivity } from '../../components/service/ServiceActivity';
import { useAppStore } from '../../stores/appStore';
import { usePOAStore, persistRegistries } from '../../stores/poaStore';
import {
  getServices,
  removeService as removeServiceCmd,
  listKvEntries,
  listFsEntries,
  readFsFile,
} from '../../tauri';
import { ServiceUpdateModal } from '../../components/service';
import { getPublicClient, getAddress } from '../../hooks/useViemClient';
import { connectToRegistry, fetchOperators } from '../../utils/evm';
import { getServiceAddress, getServiceChain, getErrorMessage, buildServiceMap } from '../../types';
import type { Service, KvEntry, FsEntry, Workflow, AllowedHostPermission } from '../../types';
import type { Address } from 'viem';
import { getRegistryKeyFromParams } from './ServicesLayout';

const REGISTRY_TABS = [
  { key: 'workflows', label: 'Workflows' },
  { key: 'components', label: 'Components' },
  { key: 'activity', label: 'Activity' },
  { key: 'operators', label: 'Operators' },
  { key: 'storage', label: 'Storage' },
];

const SERVICE_TABS = [
  { key: 'workflows', label: 'Workflows' },
  { key: 'components', label: 'Components' },
  { key: 'activity', label: 'Activity' },
  { key: 'storage', label: 'Storage' },
];

// ── Helpers ──────────────────────────────────────────────────────────────────

function decodeBase64Value(b64: string): string {
  try {
    const binary = atob(b64);
    // Check if valid UTF-8 text
    const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
    const decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    return decoded;
  } catch {
    // Fall back to hex
    const binary = atob(b64);
    return Array.from(binary)
      .map((c) => c.charCodeAt(0).toString(16).padStart(2, '0'))
      .join('');
  }
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function FileContentModal({ content }: { content: number[] }) {
  const bytes = new Uint8Array(content);
  let display: string;
  let isText = false;
  try {
    display = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    isText = true;
    // Try pretty-printing as JSON
    try {
      display = JSON.stringify(JSON.parse(display), null, 2);
    } catch {
      // leave as plain text
    }
  } catch {
    display = Array.from(bytes)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join(' ');
  }
  return (
    <div className="flex flex-col gap-2">
      <p className="text-tan-muted text-xs">{isText ? 'Text / JSON' : 'Binary (hex)'}</p>
      <pre className="text-beige-light text-xs bg-charcoal-dark p-3 rounded overflow-auto max-h-96 whitespace-pre-wrap break-all">
        {display}
      </pre>
    </div>
  );
}

// ── KV Browser ────────────────────────────────────────────────────────────────

function KvBrowser({ serviceId }: { serviceId: string }) {
  const [entries, setEntries] = useState<KvEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listKvEntries(serviceId);
      setEntries(data);
    } catch (err) {
      Toast.error(`KV fetch failed: ${getErrorMessage(err)}`);
    } finally {
      setLoading(false);
    }
  }, [serviceId]);

  useEffect(() => { load(); }, [load]);

  const toggleExpand = (i: number) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i); else next.add(i);
      return next;
    });
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h3 className="text-beige-light font-medium">KV Store</h3>
        <Button text={loading ? 'Loading…' : 'Refresh'} size="sm" disabled={loading} onClick={load} />
      </div>
      {entries.length === 0 ? (
        <p className="text-tan-muted italic text-sm">No KV entries found for this service.</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-tan-muted text-xs border-b border-charcoal-light">
                <th className="text-left py-1.5 pr-4 font-medium">Bucket</th>
                <th className="text-left py-1.5 pr-4 font-medium">Key</th>
                <th className="text-left py-1.5 font-medium">Value</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry, i) => {
                const decoded = decodeBase64Value(entry.value_b64);
                const truncated = decoded.length > 100;
                const isExpanded = expanded.has(i);
                return (
                  <tr key={i} className="border-b border-charcoal-light/40">
                    <td className="py-1.5 pr-4 text-beige-warm font-mono">{entry.bucket}</td>
                    <td className="py-1.5 pr-4 text-beige-warm font-mono">{entry.key}</td>
                    <td className="py-1.5 text-tan-muted font-mono text-xs">
                      <span className="break-all">
                        {truncated && !isExpanded ? decoded.slice(0, 100) + '…' : decoded}
                      </span>
                      {truncated && (
                        <button
                          className="ml-1 text-purple-1 hover:underline text-xs"
                          onClick={() => toggleExpand(i)}
                        >
                          {isExpanded ? 'collapse' : 'expand'}
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ── Filesystem Browser ────────────────────────────────────────────────────────

function FsBrowser({ serviceId }: { serviceId: string }) {
  const [breadcrumbs, setBreadcrumbs] = useState<string[]>([]);
  const [entries, setEntries] = useState<FsEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [fileLoading, setFileLoading] = useState<string | null>(null);

  const currentPath = breadcrumbs.join('/');

  const loadDir = useCallback(async (path: string) => {
    setLoading(true);
    try {
      const data = await listFsEntries(serviceId, path);
      setEntries(data);
    } catch (err) {
      Toast.error(`Filesystem fetch failed: ${getErrorMessage(err)}`);
    } finally {
      setLoading(false);
    }
  }, [serviceId]);

  useEffect(() => { loadDir(currentPath); }, [loadDir, currentPath]);

  const navigate = (name: string) => {
    setBreadcrumbs((prev) => [...prev, name]);
  };

  const navigateTo = (index: number) => {
    setBreadcrumbs((prev) => prev.slice(0, index + 1));
  };

  const goRoot = () => setBreadcrumbs([]);

  const handleView = async (entry: FsEntry) => {
    const filePath = currentPath ? `${currentPath}/${entry.name}` : entry.name;
    setFileLoading(entry.name);
    try {
      const data = await readFsFile(serviceId, filePath);
      Modal.open(<FileContentModal content={data} />);
    } catch (err) {
      Toast.error(`Failed to read file: ${getErrorMessage(err)}`);
    } finally {
      setFileLoading(null);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h3 className="text-beige-light font-medium">Filesystem</h3>
        <Button text={loading ? 'Loading…' : 'Refresh'} size="sm" disabled={loading} onClick={() => loadDir(currentPath)} />
      </div>

      {/* Breadcrumb */}
      <div className="flex items-center gap-1 text-sm flex-wrap">
        <button
          className="px-1.5 py-0.5 rounded bg-charcoal-light text-beige-warm text-xs hover:bg-charcoal-medium"
          onClick={goRoot}
        >
          /
        </button>
        {breadcrumbs.map((crumb, i) => (
          <span key={i} className="flex items-center gap-1">
            <span className="text-tan-muted">/</span>
            <button
              className="px-1.5 py-0.5 rounded bg-charcoal-light text-beige-warm text-xs hover:bg-charcoal-medium"
              onClick={() => navigateTo(i)}
            >
              {crumb}
            </button>
          </span>
        ))}
      </div>

      {entries.length === 0 && !loading ? (
        <p className="text-tan-muted italic text-sm">Directory is empty or not found.</p>
      ) : (
        <div className="flex flex-col gap-1">
          {entries
            .slice()
            .sort((a, b) => (a.is_dir === b.is_dir ? a.name.localeCompare(b.name) : a.is_dir ? -1 : 1))
            .map((entry) => (
              <div
                key={entry.name}
                className="flex items-center justify-between py-1.5 px-2 rounded hover:bg-charcoal-light/40 text-sm"
              >
                <button
                  className={`flex items-center gap-2 flex-1 text-left ${entry.is_dir ? 'cursor-pointer' : 'cursor-default'}`}
                  onClick={() => entry.is_dir && navigate(entry.name)}
                >
                  <span className="text-base">{entry.is_dir ? '📁' : '📄'}</span>
                  <span className="text-beige-warm font-mono">{entry.name}</span>
                  {entry.is_dir && <span className="text-tan-muted text-xs">/</span>}
                </button>
                <div className="flex items-center gap-3 ml-2">
                  {!entry.is_dir && entry.size != null && (
                    <span className="text-tan-muted text-xs">{formatFileSize(entry.size)}</span>
                  )}
                  {!entry.is_dir && (
                    <Button
                      text={fileLoading === entry.name ? '…' : 'View'}
                      size="sm"
                      disabled={fileLoading === entry.name}
                      onClick={() => handleView(entry)}
                    />
                  )}
                </div>
              </div>
            ))}
        </div>
      )}
    </div>
  );
}

// ── Components Tab ────────────────────────────────────────────────────────────

function formatHosts(hosts: AllowedHostPermission): string {
  if (hosts === 'all') return 'all';
  if (hosts === 'none') return 'none';
  return hosts.only.join(', ');
}

function PermRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-tan-muted">{label}:</span>
      <span className="text-beige-warm">{value}</span>
    </div>
  );
}

function ComponentsTab({ workflows }: { workflows: Record<string, Workflow> }) {
  const entries = Object.entries(workflows);

  if (entries.length === 0) {
    return <p className="text-tan-muted italic">No components defined.</p>;
  }

  return (
    <div className="flex flex-col gap-4">
      {entries.map(([workflowId, workflow]) => {
        const { component } = workflow;
        const { source } = component;

        let sourceType: string;
        let digest: string;
        if ('download' in source) {
          sourceType = 'Download';
          digest = source.download.digest;
        } else if ('registry' in source) {
          sourceType = 'Registry';
          digest = source.registry.digest;
        } else {
          sourceType = 'Digest';
          digest = source.digest;
        }

        const configKeys = Object.keys(component.config);

        return (
          <div key={workflowId} className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
            <div className="flex items-center gap-2 mb-3">
              <h4 className="text-beige-light font-medium">Workflow: {workflowId}</h4>
              <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-beige-warm rounded">{sourceType}</span>
            </div>

            <div className="flex flex-col gap-2 text-sm">
              <div className="flex items-baseline gap-2">
                <span className="text-tan-muted min-w-24">Digest:</span>
                <AddressDisplay address={digest} />
              </div>

              {'registry' in source && (
                <>
                  <div className="flex items-baseline gap-2">
                    <span className="text-tan-muted min-w-24">Package:</span>
                    <span className="text-beige-warm font-mono">
                      {source.registry.package}{source.registry.version ? `@${source.registry.version}` : ''}
                    </span>
                  </div>
                  {source.registry.domain && (
                    <div className="flex items-baseline gap-2">
                      <span className="text-tan-muted min-w-24">Domain:</span>
                      <span className="text-beige-warm">{source.registry.domain}</span>
                    </div>
                  )}
                </>
              )}

              {'download' in source && (
                <div className="flex items-baseline gap-2">
                  <span className="text-tan-muted min-w-24">URI:</span>
                  <AddressDisplay address={source.download.uri} />
                </div>
              )}

              <div className="border-t border-charcoal-light pt-2 mt-1">
                <p className="text-tan-muted text-xs font-medium mb-2">Permissions</p>
                <div className="grid grid-cols-2 gap-x-6 gap-y-1">
                  <PermRow label="HTTP Hosts" value={formatHosts(component.permissions.allowed_http_hosts)} />
                  <PermRow label="File System" value={component.permissions.file_system ? 'yes' : 'no'} />
                  <PermRow label="Raw Sockets" value={component.permissions.raw_sockets ? 'yes' : 'no'} />
                  <PermRow label="DNS Resolution" value={component.permissions.dns_resolution ? 'yes' : 'no'} />
                </div>
              </div>

              {(component.fuel_limit != null || component.time_limit_seconds != null) && (
                <div className="border-t border-charcoal-light pt-2 mt-1">
                  <p className="text-tan-muted text-xs font-medium mb-2">Resource Limits</p>
                  <div className="grid grid-cols-2 gap-x-6 gap-y-1">
                    {component.fuel_limit != null && (
                      <PermRow label="Fuel" value={component.fuel_limit.toLocaleString()} />
                    )}
                    {component.time_limit_seconds != null && (
                      <PermRow label="Time" value={`${component.time_limit_seconds}s`} />
                    )}
                  </div>
                </div>
              )}

              {configKeys.length > 0 && (
                <div className="border-t border-charcoal-light pt-2 mt-1">
                  <p className="text-tan-muted text-xs font-medium mb-2">Config</p>
                  <div className="flex flex-wrap gap-1">
                    {configKeys.map((k) => (
                      <span key={k} className="px-1.5 py-0.5 text-xs bg-charcoal-light text-beige-warm rounded font-mono">{k}</span>
                    ))}
                  </div>
                </div>
              )}

              {component.env_keys.length > 0 && (
                <div className="border-t border-charcoal-light pt-2 mt-1">
                  <p className="text-tan-muted text-xs font-medium mb-2">Env Keys</p>
                  <div className="flex flex-wrap gap-1">
                    {component.env_keys.map((k) => (
                      <span key={k} className="px-1.5 py-0.5 text-xs bg-charcoal-light text-beige-warm rounded font-mono">{k}</span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ── Storage Tab ───────────────────────────────────────────────────────────────

function StorageTab({ service, serviceId }: { service: Service; serviceId: string }) {
  const hasFs = Object.values(service.workflows).some(
    (wf) => wf.component.permissions.file_system
  );

  return (
    <div className="flex flex-col gap-6">
      <KvBrowser serviceId={serviceId} />
      {hasFs && (
        <>
          <div className="border-t border-charcoal-light" />
          <FsBrowser serviceId={serviceId} />
        </>
      )}
    </div>
  );
}

function ConfirmModal({
  title,
  message,
  confirmLabel,
  confirmColor,
  onConfirm,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  confirmColor: 'red' | 'primary' | 'purple';
  onConfirm: () => Promise<void>;
}) {
  const [loading, setLoading] = useState(false);

  const handleConfirm = async () => {
    setLoading(true);
    try {
      await onConfirm();
      Modal.close();
    } catch {
      // errors are handled in onConfirm
      Modal.close();
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <h3 className="text-beige-light text-lg font-semibold">{title}</h3>
      <p className="text-tan-muted">{message}</p>
      <div className="flex gap-2 justify-end">
        <Button text="Cancel" size="sm" onClick={() => Modal.close()} />
        <Button
          text={loading ? '...' : confirmLabel}
          size="sm"
          color={confirmColor}
          disabled={loading}
          onClick={handleConfirm}
        />
      </div>
    </div>
  );
}

export function ServiceDetailPage() {
  const { chainId, address } = useParams<{ chainId: string; address: string }>();
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState('workflows');
  const [refreshing, setRefreshing] = useState(false);

  const services = useAppStore((state) => state.services);
  const setServices = useAppStore((state) => state.setServices);
  const removeServiceFromStore = useAppStore((state) => state.removeService);
  const registries = usePOAStore((state) => state.registries);
  const { removeRegistry, updateRegistryInfo, updateRegistryOperators, updateRegistryOwnership } = usePOAStore();

  if (!chainId || !address) {
    return <div className="text-red-3">Invalid URL parameters.</div>;
  }

  const registryKey = getRegistryKeyFromParams(chainId, address);
  const registry = registries.get(registryKey) ?? null;

  // Find matching service by address
  let serviceHashId: string | null = null;
  let service: Service | null = null;
  for (const [hashId, s] of services.entries()) {
    if (getServiceAddress(s.manager).toLowerCase() === address.toLowerCase()) {
      serviceHashId = hashId;
      service = s;
      break;
    }
  }

  if (!registry && !service) {
    return (
      <div className="flex flex-col gap-4">
        <div className="text-tan-muted">Service not found for {address}</div>
        <Button text="Back to Services" size="sm" onClick={() => navigate('/services')} />
      </div>
    );
  }

  const chainKey = registry?.chainKey ?? getServiceChain(service!.manager);
  const contractAddress = (registry?.address ?? address) as Address;

  const refreshServices = async () => {
    const servicesData = await getServices();
    setServices(await buildServiceMap(servicesData));
  };

  const handleRefresh = async () => {
    if (!registry) return;
    setRefreshing(true);
    try {
      const publicClient = getPublicClient(registry.rpcUrl, registry.chainId);
      const userAddress = await getAddress();
      const [info, operators] = await Promise.all([
        connectToRegistry(publicClient, registry.address),
        fetchOperators(publicClient, registry.address),
      ]);
      updateRegistryInfo(registryKey, info);
      updateRegistryOperators(registryKey, operators);
      updateRegistryOwnership(registryKey, info.owner.toLowerCase() === userAddress.toLowerCase());
      await refreshServices();
    } catch (err) {
      Toast.error(`Failed to refresh: ${getErrorMessage(err)}`);
    } finally {
      setRefreshing(false);
    }
  };

  const handleDelete = () => {
    Modal.open(
      <ConfirmModal
        title={registry ? 'Delete Registry' : 'Delete Service'}
        message={
          registry
            ? 'Remove this registry from the app? Any running service will be stopped. The contract remains on-chain and can be re-added later.'
            : 'Remove this service from WAVS? The component will stop executing.'
        }
        confirmLabel="Delete"
        confirmColor="red"
        onConfirm={async () => {
          try {
            if (service) {
              await removeServiceCmd(service.manager);
              if (serviceHashId) removeServiceFromStore(serviceHashId);
            }
            if (registry) {
              removeRegistry(registryKey);
              await persistRegistries();
            }
            navigate('/services');
          } catch (err) {
            Toast.error(`Failed to delete: ${getErrorMessage(err)}`);
          }
        }}
      />
    );
  };

  const handlePauseResume = () => {
    if (!service || !registry) return;
    const newStatus = service.status === 'active' ? 'paused' : 'active';
    const updatedService = { ...service, status: newStatus as Service['status'] };
    const action = newStatus === 'paused' ? 'Pause service' : 'Resume service';

    Modal.open(
      <ServiceUpdateModal
        updatedService={updatedService}
        description={action}
        rpcUrl={registry.rpcUrl}
        chainId={registry.chainId}
        contractAddress={contractAddress}
      />,
    );
  };

  const isPaused = service?.status === 'paused';
  const baseTabs = registry ? REGISTRY_TABS : SERVICE_TABS;
  // Only show Storage tab when there is an active service
  const tabs = service ? baseTabs : baseTabs.filter((t) => t.key !== 'storage');

  return (
    <div className="flex flex-col gap-6">
      {/* Header Section */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        {/* Title row */}
        <div className="flex items-center gap-2 flex-wrap mb-3">
          <h2 className="text-beige-light text-xl font-semibold">
            {service?.name ?? `${chainKey} Registry`}
          </h2>
          {service && !isPaused && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-green-700 text-green-100 rounded">
              Active
            </span>
          )}
          {service && isPaused && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-yellow-700 text-yellow-100 rounded">
              Paused
            </span>
          )}
          {!service && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-tan-muted rounded">
              Not registered
            </span>
          )}
          {registry?.isOwner && (
            <span className="px-1.5 py-0.5 text-xs font-medium bg-purple-1 text-cream-light rounded">
              Owner
            </span>
          )}
        </div>

        {/* Contract info */}
        <div className="grid grid-cols-2 gap-3 mb-4 text-sm">
          <div className="flex flex-col gap-1">
            <span className="text-tan-muted text-xs font-medium">Chain</span>
            <span className="text-beige-warm">{chainKey}</span>
          </div>
          <div className="flex flex-col gap-1">
            <span className="text-tan-muted text-xs font-medium">Address</span>
            <AddressDisplay address={contractAddress} full />
          </div>
          {registry?.info && (
            <>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Owner</span>
                <AddressDisplay address={registry.info.owner} full />
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Total Weight</span>
                <span className="text-beige-warm">{registry.info.totalWeight.toString()}</span>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Threshold</span>
                <span className="text-beige-warm">{registry.info.thresholdWeight.toString()}</span>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-tan-muted text-xs font-medium">Quorum</span>
                <span className="text-beige-warm">
                  {registry.info.quorumNumerator.toString()}/{registry.info.quorumDenominator.toString()}
                  {' '}({((Number(registry.info.quorumNumerator) / Number(registry.info.quorumDenominator)) * 100).toFixed(1)}%)
                </span>
              </div>
              {registry.info.serviceUri && (
                <div className="flex flex-col gap-1 col-span-2">
                  <span className="text-tan-muted text-xs font-medium">Service URI</span>
                  <span className="text-beige-warm text-xs break-all">{registry.info.serviceUri}</span>
                </div>
              )}
            </>
          )}
        </div>

        {/* Actions — primary left, destructive right */}
        <div className="flex items-center justify-between gap-2 flex-wrap">
          {/* Primary actions */}
          <div className="flex items-center gap-2 flex-wrap">
            {service ? (
              <>
                <Button text="Edit" size="sm" color="purple" onClick={() => navigate(`/services/${chainId}/${address}/edit`)} />
                <Button
                  text={isPaused ? 'Resume' : 'Pause'}
                  size="sm"
                  variant="outline"
                  onClick={handlePauseResume}
                />
              </>
            ) : (
              <Button text="Register Service" size="sm" color="purple" onClick={() => navigate(`/services/new?registry=${registryKey}`)} />
            )}
          </div>

          {/* Secondary / destructive actions */}
          <div className="flex items-center gap-2 flex-wrap">
            {registry && (
              <Button
                text={refreshing ? 'Refreshing...' : 'Refresh'}
                size="sm"
                disabled={refreshing}
                onClick={handleRefresh}
              />
            )}
            {registry?.isOwner && <OwnerActionsMenu registryKey={registryKey} />}
            <Button text="Delete" size="sm" color="red" variant="outline" onClick={handleDelete} />
          </div>
        </div>
      </div>

      {/* Tabs */}
      <Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

      {/* Tab content */}
      <div>
        {activeTab === 'workflows' && (
          service ? (
            <WorkflowViewer workflows={service.workflows} />
          ) : (
            <p className="text-tan-muted italic">Register a service to see its workflows.</p>
          )
        )}
        {activeTab === 'components' && (
          service ? (
            <ComponentsTab workflows={service.workflows} />
          ) : (
            <p className="text-tan-muted italic">Register a service to see its components.</p>
          )
        )}
        {activeTab === 'activity' && service && serviceHashId && (
          <ServiceActivity
            serviceId={serviceHashId}
            workflowIds={Object.keys(service.workflows)}
          />
        )}
        {activeTab === 'activity' && !service && (
          <p className="text-tan-muted italic">Register a service to see its activity.</p>
        )}
        {activeTab === 'operators' && registry && (
          <OperatorList registryKey={registryKey} />
        )}
        {activeTab === 'storage' && service && serviceHashId && (
          <StorageTab service={service} serviceId={serviceHashId} />
        )}
        {activeTab === 'storage' && !service && (
          <p className="text-tan-muted italic">Register a service to browse its storage.</p>
        )}
      </div>
    </div>
  );
}
