import { Button } from '../atoms';
import { RegistrySelector } from '../poa';
import { useServiceBuilderStore } from '../../stores/serviceBuilderStore';
import { usePOAStore } from '../../stores/poaStore';

export function ContractStep() {
  const selectedRegistryKey = useServiceBuilderStore((s) => s.selectedRegistryKey);
  const setSelectedRegistry = useServiceBuilderStore((s) => s.setSelectedRegistry);

  const registries = usePOAStore((s) => s.registries);

  const selectedRegistry = selectedRegistryKey ? registries.get(selectedRegistryKey) ?? null : null;

  const handleRegistryAdded = (key: string) => {
    setSelectedRegistry(key);
  };

  const handleClearSelection = () => {
    setSelectedRegistry(null);
  };

  return (
    <div className="flex flex-col gap-6">
      {/* If a registry is already selected, show summary */}
      {selectedRegistry ? (
        <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
          <div className="flex items-center justify-between mb-3">
            <h4 className="text-beige-warm text-sm font-medium">Selected Contract</h4>
            <Button text="Change" size="sm" variant="outline" onClick={handleClearSelection} />
          </div>
          <div className="grid grid-cols-2 gap-2 text-sm">
            <div>
              <span className="text-tan-muted">Chain: </span>
              <span className="text-beige-warm">{selectedRegistry.chainKey}</span>
            </div>
            <div>
              <span className="text-tan-muted">Chain ID: </span>
              <span className="text-beige-warm">{selectedRegistry.chainId}</span>
            </div>
            <div className="col-span-2">
              <span className="text-tan-muted">Address: </span>
              <span className="text-beige-warm font-mono text-xs">{selectedRegistry.address}</span>
            </div>
            {selectedRegistry.isOwner && (
              <div className="col-span-2">
                <span className="px-1.5 py-0.5 text-xs font-medium bg-purple-1 text-cream-light rounded">
                  Owner
                </span>
              </div>
            )}
          </div>
        </div>
      ) : (
        <RegistrySelector onRegistryAdded={handleRegistryAdded} />
      )}
    </div>
  );
}
