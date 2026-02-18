import { useState, useMemo } from 'react';
import { Button, TextArea } from '../atoms';
import { useServiceBuilderStore } from '../../stores/serviceBuilderStore';
import { usePOAStore } from '../../stores/poaStore';
import type { Service, ServiceManager } from '../../types';

export function ServiceReview() {
  const buildServiceJson = useServiceBuilderStore((s) => s.buildServiceJson);
  const selectedRegistryKey = useServiceBuilderStore((s) => s.selectedRegistryKey);
  const manualChain = useServiceBuilderStore((s) => s.manualChain);
  const manualAddress = useServiceBuilderStore((s) => s.manualAddress);
  const registries = usePOAStore((s) => s.registries);

  const [editMode, setEditMode] = useState(false);
  const [editedJson, setEditedJson] = useState('');

  // Resolve the service manager
  const serviceManager: ServiceManager | null = useMemo(() => {
    if (selectedRegistryKey) {
      const registry = registries.get(selectedRegistryKey);
      if (registry) {
        // Determine if EVM or Cosmos based on chain key
        if (registry.chainKey.startsWith('cosmos:')) {
          return { cosmos: { chain: registry.chainKey, address: registry.address } };
        }
        return { evm: { chain: registry.chainKey, address: registry.address } };
      }
    }
    if (manualChain && manualAddress) {
      if (manualAddress.startsWith('0x')) {
        return { evm: { chain: manualChain, address: manualAddress } };
      }
      return { cosmos: { chain: manualChain, address: manualAddress } };
    }
    return null;
  }, [selectedRegistryKey, registries, manualChain, manualAddress]);

  const service = useMemo(() => {
    const built = buildServiceJson();
    if (!built || !serviceManager) return null;
    return { ...built, manager: serviceManager };
  }, [buildServiceJson, serviceManager]);

  const jsonString = useMemo(() => {
    if (!service) return '';
    return JSON.stringify(service, null, 2);
  }, [service]);

  const warnings: string[] = [];
  if (!service) {
    warnings.push('Service JSON could not be constructed. Check that all required fields are filled.');
  }
  if (!serviceManager) {
    warnings.push('No service manager selected. Select a POA registry or enter chain/address manually.');
  }

  // Validate edited JSON
  const editedService = useMemo((): Service | null => {
    if (!editMode || !editedJson) return null;
    try {
      return JSON.parse(editedJson) as Service;
    } catch {
      return null;
    }
  }, [editMode, editedJson]);

  const toggleEditMode = () => {
    if (!editMode) {
      setEditedJson(jsonString);
    }
    setEditMode(!editMode);
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h3 className="text-beige-light text-lg font-semibold">Review Service JSON</h3>
        <Button
          text={editMode ? 'Preview' : 'Edit JSON'}
          color="primary"
          size="sm"
          variant="outline"
          onClick={toggleEditMode}
          disabled={!service}
        />
      </div>

      {warnings.length > 0 && (
        <div className="flex flex-col gap-1 p-3 rounded bg-charcoal-dark border border-yellow-700">
          {warnings.map((w, i) => (
            <p key={i} className="text-yellow-400 text-sm">{w}</p>
          ))}
        </div>
      )}

      {editMode ? (
        <div className="flex flex-col gap-2">
          <TextArea
            value={editedJson}
            onChange={setEditedJson}
            rows={20}
          />
          {editedJson && !editedService && (
            <p className="text-red-3 text-sm">Invalid JSON</p>
          )}
        </div>
      ) : (
        <pre className="p-4 rounded bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm whitespace-pre-wrap overflow-x-auto max-h-[60vh] overflow-y-auto">
          {jsonString || 'No service JSON generated yet.'}
        </pre>
      )}
    </div>
  );
}
