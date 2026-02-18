import { useState, useEffect } from 'react';
import type { Address } from 'viem';
import { Button, Expander, Modal } from '../components/atoms';
import {
  RegistrySelector,
  RegistryInfo,
  OperatorList,
  OwnerActions,
} from '../components/poa';
import { ServiceBuilder } from '../components/service';
import { useAppStore } from '../stores/appStore';
import { usePOAStore, getRegistryKey, persistRegistries, type ConnectedRegistry } from '../stores/poaStore';
import { useServiceBuilderStore } from '../stores/serviceBuilderStore';
import { getServices, getSettings } from '../tauri';
import { getPublicClient, getAddress } from '../hooks/useViemClient';
import { connectToRegistry, fetchOperators } from '../utils/evm';
import type { Service } from '../types';
import { getServiceId, getServiceAddress, getErrorMessage } from '../types';

export function Services() {
  const services = useAppStore((state) => state.services);
  const setServices = useAppStore((state) => state.setServices);

  const {
    registries,
    addRegistry,
    updateRegistryInfo,
    updateRegistryOperators,
    updateRegistryOwnership,
    removeRegistry,
  } = usePOAStore();

  const [loading, setLoading] = useState(true);
  const [showBuilder, setShowBuilder] = useState(false);
  const [showAddContract, setShowAddContract] = useState(false);
  const [refreshingKey, setRefreshingKey] = useState<string | null>(null);
  const reset = useServiceBuilderStore((s) => s.reset);
  const setSelectedRegistry = useServiceBuilderStore((s) => s.setSelectedRegistry);

  // Load saved registries + WAVS services on mount
  useEffect(() => {
    const init = async () => {
      try {
        // Load services and registries in parallel
        const [servicesData, settings] = await Promise.all([
          getServices(),
          getSettings(),
        ]);

        setServices(servicesData);

        // Load saved registries
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
    setShowBuilder(true);
  };

  const handleCloseBuilder = () => {
    setShowBuilder(false);
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

  const handleRemove = async (key: string) => {
    removeRegistry(key);
    await persistRegistries();
  };

  if (loading) {
    return (
      <div className="text-lg text-beige-warm">Loading services...</div>
    );
  }

  const registryList = Array.from(registries.entries());
  const serviceList = Array.from(services.values());

  if (showBuilder) {
    return (
      <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
        <ServiceBuilder onClose={handleCloseBuilder} />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-beige-light text-xl font-semibold">Services</h2>
        <div className="flex items-center gap-2">
          <Button
            text="Add Contract"
            size="sm"
            color="purple"
            variant="outline"
            onClick={() => setShowAddContract(!showAddContract)}
          />
          <Button
            text="Create Service"
            color="purple"
            onClick={() => handleOpenBuilder()}
          />
        </div>
      </div>

      {/* Add Contract form (toggle) */}
      {showAddContract && (
        <RegistrySelector onComplete={() => setShowAddContract(false)} />
      )}

      {/* No contracts message + selector */}
      {registryList.length === 0 && !showAddContract && (
        <RegistrySelector />
      )}

      {/* Service Contracts section */}
      {registryList.length > 0 && (
        <div className="flex flex-col gap-3">
          <h3 className="text-beige-warm text-sm font-medium uppercase tracking-wider">
            Service Contracts ({registryList.length})
          </h3>
          {registryList.map(([key, registry]) => (
            <ServiceContractCard
              key={key}
              registryKey={key}
              registry={registry}
              refreshing={refreshingKey === key}
              services={serviceList}
              onRefresh={() => handleRefresh(key)}
              onRemove={() => handleRemove(key)}
              onBuildService={() => handleOpenBuilder(key)}
            />
          ))}
        </div>
      )}

      {/* WAVS Registered Services section */}
      <div className="flex flex-col gap-3">
        <h3 className="text-beige-warm text-sm font-medium uppercase tracking-wider">
          Registered Services ({serviceList.length})
        </h3>
        {serviceList.length === 0 ? (
          <div className="p-8 text-center text-tan-muted italic">
            No services registered yet. Click "Create Service" to build and deploy a new service.
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            {serviceList.map((service) => (
              <ServiceItem key={getServiceId(service)} service={service} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function ServiceContractCard({
  registryKey,
  registry,
  refreshing,
  services,
  onRefresh,
  onRemove,
  onBuildService,
}: {
  registryKey: string;
  registry: ConnectedRegistry;
  refreshing: boolean;
  services: Service[];
  onRefresh: () => void;
  onRemove: () => void;
  onBuildService: () => void;
}) {
  // Check if this contract address matches any WAVS-registered service
  const isWavsRegistered = services.some(
    (s) => getServiceAddress(s.manager).toLowerCase() === registry.address.toLowerCase()
  );

  const label = (
    <div className="flex items-center gap-2">
      <span>{registry.chainKey} - {registry.address.slice(0, 8)}...{registry.address.slice(-6)}</span>
      {registry.isOwner && (
        <span className="px-1.5 py-0.5 text-xs font-medium bg-purple-1 text-cream-light rounded">
          Owner
        </span>
      )}
      {isWavsRegistered && (
        <span className="px-1.5 py-0.5 text-xs font-medium bg-green-700 text-green-100 rounded">
          WAVS Registered
        </span>
      )}
    </div>
  );

  return (
    <Expander label={label}>
      {/* Action buttons */}
      <div className="flex items-center gap-2 mb-4">
        <Button
          text="Build Service Definition"
          size="sm"
          color="purple"
          onClick={onBuildService}
        />
        <Button
          text={refreshing ? 'Refreshing...' : 'Refresh'}
          size="sm"
          disabled={refreshing}
          onClick={onRefresh}
        />
        <Button
          text="Remove"
          size="sm"
          color="red"
          variant="outline"
          onClick={onRemove}
        />
      </div>

      {/* Contract details */}
      <div className="flex flex-col gap-4">
        <RegistryInfo registryKey={registryKey} />
        <OperatorList registryKey={registryKey} />
        <OwnerActions registryKey={registryKey} />
      </div>
    </Expander>
  );
}

function ServiceItem({ service }: { service: Service }) {
  const content = (
    <pre className="text-sm whitespace-pre-wrap">
      {JSON.stringify(service, null, 2)}
    </pre>
  );

  return (
    <Expander label={service.name}>
      {content}
    </Expander>
  );
}
