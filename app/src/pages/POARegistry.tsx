import { useState } from 'react';
import { Button, Dropdown, type DropdownOption } from '../components/atoms';
import {
  RegistrySelector,
  RegistryInfo,
  OperatorList,
  OwnerActions,
} from '../components/poa';
import { usePOAStore, getRegistryKey } from '../stores/poaStore';
import { getPublicClient, getAddress } from '../hooks/useViemClient';
import { connectToRegistry, fetchOperators } from '../utils/evm';

export function POARegistry() {
  const {
    registries,
    activeRegistryKey,
    setActiveRegistry,
    updateRegistryInfo,
    updateRegistryOperators,
    updateRegistryOwnership,
    removeRegistry,
    getActiveRegistry,
  } = usePOAStore();

  const [refreshing, setRefreshing] = useState(false);

  const activeRegistry = getActiveRegistry();
  const registryList = Array.from(registries.entries());

  // Create dropdown options from connected registries
  const registryOptions: DropdownOption<string>[] = registryList.map(([key, reg]) => ({
    label: `${reg.chainKey} - ${reg.address.slice(0, 8)}...${reg.address.slice(-6)}`,
    value: key,
  }));

  const handleRefresh = async () => {
    if (!activeRegistry) return;

    setRefreshing(true);
    try {
      const publicClient = getPublicClient(activeRegistry.rpcUrl, activeRegistry.chainId);
      const userAddress = await getAddress();

      const [info, operators] = await Promise.all([
        connectToRegistry(publicClient, activeRegistry.address),
        fetchOperators(publicClient, activeRegistry.address),
      ]);

      const key = getRegistryKey(activeRegistry.chainId, activeRegistry.address);
      updateRegistryInfo(key, info);
      updateRegistryOperators(key, operators);
      updateRegistryOwnership(key, info.owner.toLowerCase() === userAddress.toLowerCase());
    } catch (err) {
      console.error('Failed to refresh registry:', err);
    } finally {
      setRefreshing(false);
    }
  };

  const handleDisconnect = () => {
    if (activeRegistryKey) {
      removeRegistry(activeRegistryKey);
    }
  };

  // If no registries connected, show the selector
  if (registryList.length === 0) {
    return (
      <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
        <RegistrySelector />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {/* Registry Selector Header */}
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <h2 className="text-xl font-semibold text-beige-light">POA Registry</h2>
          {registryList.length > 1 && (
            <Dropdown
              options={registryOptions}
              value={activeRegistryKey ?? undefined}
              onChange={setActiveRegistry}
              size="sm"
            />
          )}
        </div>
        <div className="flex items-center gap-2">
          <Button
            text={refreshing ? 'Refreshing...' : 'Refresh'}
            size="sm"
            disabled={refreshing || !activeRegistry}
            onClick={handleRefresh}
          />
          <Button
            text="Add Registry"
            size="sm"
            color="purple"
            onClick={() => setActiveRegistry(null)}
          />
          {activeRegistry && (
            <Button
              text="Disconnect"
              size="sm"
              color="red"
              variant="outline"
              onClick={handleDisconnect}
            />
          )}
        </div>
      </div>

      {/* Show selector if no active registry or if user clicked "Add Registry" */}
      {!activeRegistry && (
        <RegistrySelector />
      )}

      {/* Show registry details if connected */}
      {activeRegistry && (
        <div className="flex flex-col gap-6">
          <RegistryInfo />
          <OperatorList />
          <OwnerActions />
        </div>
      )}
    </div>
  );
}
