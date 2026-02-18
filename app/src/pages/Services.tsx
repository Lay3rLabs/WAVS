import { useState, useEffect } from 'react';
import type { Address } from 'viem';
import { Button, Expander, Modal } from '../components/atoms';
import {
  RegistryInfo,
  OperatorList,
  OwnerActions,
} from '../components/poa';
import { ServiceBuilder, ServiceEditor } from '../components/service';
import { useAppStore } from '../stores/appStore';
import { usePOAStore, getRegistryKey, persistRegistries, type ConnectedRegistry } from '../stores/poaStore';
import { useServiceBuilderStore } from '../stores/serviceBuilderStore';
import { getServices, getSettings, removeService as removeServiceCmd } from '../tauri';
import { getPublicClient, getAddress } from '../hooks/useViemClient';
import { connectToRegistry, fetchOperators } from '../utils/evm';
import type { Service } from '../types';
import { getServiceId, getServiceAddress, getErrorMessage } from '../types';

type PageView = 'list' | 'builder' | 'edit';

export function Services() {
  const services = useAppStore((state) => state.services);
  const setServices = useAppStore((state) => state.setServices);
  const removeServiceFromStore = useAppStore((state) => state.removeService);

  const {
    registries,
    addRegistry,
    updateRegistryInfo,
    updateRegistryOperators,
    updateRegistryOwnership,
    removeRegistry,
  } = usePOAStore();

  const [loading, setLoading] = useState(true);
  const [view, setView] = useState<PageView>('list');
  const [editingService, setEditingService] = useState<{ service: Service; registryKey: string } | null>(null);
  const [refreshingKey, setRefreshingKey] = useState<string | null>(null);
  const reset = useServiceBuilderStore((s) => s.reset);
  const setSelectedRegistry = useServiceBuilderStore((s) => s.setSelectedRegistry);

  // Load saved registries + WAVS services on mount
  useEffect(() => {
    const init = async () => {
      try {
        const [servicesData, settings] = await Promise.all([
          getServices(),
          getSettings(),
        ]);

        setServices(servicesData);

        for (const saved of settings.saved_registries) {
          const key = getRegistryKey(saved.chain_id, saved.address as Address);
          if (registries.has(key)) continue;

          const publicClient = getPublicClient(saved.rpc_url, saved.chain_id);
          const userAddress = await getAddress();
          const [info, operators] = await Promise.all([
            connectToRegistry(publicClient, saved.address as Address),
            fetchOperators(publicClient, saved.address as Address),
          ]);

          addRegistry({
            chainId: saved.chain_id,
            chainKey: saved.chain_key,
            rpcUrl: saved.rpc_url,
            address: saved.address as Address,
            info,
            operators,
            isOwner: info.owner.toLowerCase() === userAddress.toLowerCase(),
          });
        }
      } catch (err) {
        console.error('Failed to load services:', err);
        Modal.openError(`Failed to load services: ${getErrorMessage(err)}`);
      } finally {
        setLoading(false);
      }
    };

    init();
  }, []);

  const handleOpenBuilder = (preselectedRegistryKey?: string) => {
    reset();
    if (preselectedRegistryKey) {
      setSelectedRegistry(preselectedRegistryKey);
    }
    setView('builder');
  };

  const handleCloseBuilder = () => {
    setView('list');
  };

  const handleEditService = (service: Service, registryKey: string) => {
    setEditingService({ service, registryKey });
    setView('edit');
  };

  const handleCloseEditor = () => {
    setEditingService(null);
    setView('list');
  };

  const handleRefresh = async (key: string) => {
    const registry = registries.get(key);
    if (!registry) return;

    setRefreshingKey(key);
    try {
      const publicClient = getPublicClient(registry.rpcUrl, registry.chainId);
      const userAddress = await getAddress();

      const [info, operators] = await Promise.all([
        connectToRegistry(publicClient, registry.address),
        fetchOperators(publicClient, registry.address),
      ]);

      updateRegistryInfo(key, info);
      updateRegistryOperators(key, operators);
      updateRegistryOwnership(key, info.owner.toLowerCase() === userAddress.toLowerCase());
    } catch (err) {
      console.error('Failed to refresh registry:', err);
      Modal.openError(`Failed to refresh: ${getErrorMessage(err)}`);
    } finally {
      setRefreshingKey(null);
    }
  };

  const handleDisconnect = async (key: string) => {
    removeRegistry(key);
    await persistRegistries();
  };

  const handleRemoveService = async (service: Service) => {
    const confirmed = window.confirm(
      `Remove "${service.name}" from WAVS? This will stop all workflows. The contract remains on-chain.`
    );
    if (!confirmed) return;
    try {
      await removeServiceCmd(service.manager);
      removeServiceFromStore(getServiceId(service));
      Modal.openInfo('Service removed from WAVS.');
    } catch (err) {
      Modal.openError(`Failed to remove service: ${getErrorMessage(err)}`);
    }
  };

  if (loading) {
    return (
      <div className="text-lg text-beige-warm">Loading services...</div>
    );
  }

  // View routing
  if (view === 'builder') {
    return (
      <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
        <ServiceBuilder onClose={handleCloseBuilder} />
      </div>
    );
  }

  if (view === 'edit' && editingService) {
    return (
      <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
        <ServiceEditor
          service={editingService.service}
          registryKey={editingService.registryKey}
          onClose={handleCloseEditor}
        />
      </div>
    );
  }

  // Unified list: merge registries with services
  const serviceList = Array.from(services.values());
  const registryList = Array.from(registries.entries());

  const unifiedEntries = registryList.map(([key, registry]) => {
    const service = serviceList.find(
      (s) => getServiceAddress(s.manager).toLowerCase() === registry.address.toLowerCase()
    ) ?? null;
    return { registryKey: key, registry, service };
  });

  // Empty state
  if (unifiedEntries.length === 0) {
    return (
      <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
        <div className="flex items-center justify-between">
          <h2 className="text-beige-light text-xl font-semibold">Services</h2>
        </div>
        <div className="flex flex-col items-center justify-center gap-4 py-16">
          <p className="text-tan-muted text-center">
            No services yet. Create your first service to get started.
          </p>
          <Button
            text="New Service"
            color="purple"
            onClick={() => handleOpenBuilder()}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-beige-light text-xl font-semibold">Services</h2>
        <Button
          text="New Service"
          color="purple"
          onClick={() => handleOpenBuilder()}
        />
      </div>

      {/* Unified list */}
      <div className="flex flex-col gap-3">
        {unifiedEntries.map(({ registryKey, registry, service }) => (
          <ServiceCard
            key={registryKey}
            registryKey={registryKey}
            registry={registry}
            service={service}
            refreshing={refreshingKey === registryKey}
            onRefresh={() => handleRefresh(registryKey)}
            onDisconnect={() => handleDisconnect(registryKey)}
            onRegisterService={() => handleOpenBuilder(registryKey)}
            onEditService={() => service && handleEditService(service, registryKey)}
            onRemoveService={() => service && handleRemoveService(service)}
          />
        ))}
      </div>
    </div>
  );
}

function ServiceCard({
  registryKey,
  registry,
  service,
  refreshing,
  onRefresh,
  onDisconnect,
  onRegisterService,
  onEditService,
  onRemoveService,
}: {
  registryKey: string;
  registry: ConnectedRegistry;
  service: Service | null;
  refreshing: boolean;
  onRefresh: () => void;
  onDisconnect: () => void;
  onRegisterService: () => void;
  onEditService: () => void;
  onRemoveService: () => void;
}) {
  const [showJson, setShowJson] = useState(false);

  const operatorCount = registry.operators.length;

  const label = (
    <div className="flex items-center gap-2 flex-wrap">
      {service ? (
        <>
          <span className="text-beige-light font-medium">{service.name}</span>
          <span className="px-1.5 py-0.5 text-xs font-medium bg-green-700 text-green-100 rounded">
            Active
          </span>
        </>
      ) : (
        <>
          <span className="text-beige-warm">
            {registry.chainKey} &middot; {registry.address.slice(0, 8)}...{registry.address.slice(-6)}
          </span>
          <span className="px-1.5 py-0.5 text-xs font-medium bg-charcoal-light text-tan-muted rounded">
            Not registered
          </span>
        </>
      )}
      {registry.isOwner && (
        <span className="px-1.5 py-0.5 text-xs font-medium bg-purple-1 text-cream-light rounded">
          Owner
        </span>
      )}
      {operatorCount > 0 && (
        <span className="text-xs text-tan-muted">
          {operatorCount} operator{operatorCount !== 1 ? 's' : ''}
        </span>
      )}
    </div>
  );

  const subtitle = service ? (
    <div className="text-xs text-tan-muted mt-0.5">
      {registry.chainKey} &middot; {registry.address.slice(0, 8)}...{registry.address.slice(-6)}
    </div>
  ) : null;

  return (
    <Expander label={<>{label}{subtitle}</>}>
      {/* Action buttons */}
      <div className="flex items-center gap-2 mb-4 flex-wrap">
        {service ? (
          <>
            <Button text="Edit" size="sm" color="purple" onClick={onEditService} />
            <Button
              text={showJson ? 'Hide JSON' : 'View JSON'}
              size="sm"
              variant="outline"
              onClick={() => setShowJson(!showJson)}
            />
            <Button text="Remove from WAVS" size="sm" color="red" variant="outline" onClick={onRemoveService} />
          </>
        ) : (
          <Button text="Register Service" size="sm" color="purple" onClick={onRegisterService} />
        )}
        <Button
          text={refreshing ? 'Refreshing...' : 'Refresh'}
          size="sm"
          disabled={refreshing}
          onClick={onRefresh}
        />
        <Button text="Disconnect" size="sm" color="red" variant="outline" onClick={onDisconnect} />
      </div>

      {/* JSON viewer (toggle) */}
      {showJson && service && (
        <div className="mb-4 p-3 rounded bg-charcoal-dark border border-charcoal-light overflow-x-auto">
          <pre className="text-xs text-beige-warm font-mono whitespace-pre-wrap">
            {JSON.stringify(service, null, 2)}
          </pre>
        </div>
      )}

      {/* Contract details */}
      <div className="flex flex-col gap-4">
        <RegistryInfo registryKey={registryKey} />
        <OperatorList registryKey={registryKey} />

        {/* Owner Actions - collapsed by default */}
        {registry.isOwner && (
          <Expander label="Owner Actions" defaultExpanded={false}>
            <OwnerActions registryKey={registryKey} />
          </Expander>
        )}
      </div>
    </Expander>
  );
}
