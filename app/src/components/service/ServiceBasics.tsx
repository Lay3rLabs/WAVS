import { TextInput, Dropdown, type DropdownOption } from '../atoms';
import { useServiceBuilderStore } from '../../stores/serviceBuilderStore';
import { usePOAStore, type ConnectedRegistry } from '../../stores/poaStore';

export function ServiceBasics() {
  const name = useServiceBuilderStore((s) => s.name);
  const setName = useServiceBuilderStore((s) => s.setName);
  const selectedRegistryKey = useServiceBuilderStore((s) => s.selectedRegistryKey);
  const setSelectedRegistry = useServiceBuilderStore((s) => s.setSelectedRegistry);
  const manualChain = useServiceBuilderStore((s) => s.manualChain);
  const setManualChain = useServiceBuilderStore((s) => s.setManualChain);
  const manualAddress = useServiceBuilderStore((s) => s.manualAddress);
  const setManualAddress = useServiceBuilderStore((s) => s.setManualAddress);

  const registries = usePOAStore((s) => s.registries);

  const registryEntries = Array.from(registries.entries());
  const registryOptions: DropdownOption<string>[] = registryEntries.map(([key, reg]) => ({
    label: `${reg.chainKey} - ${reg.address.slice(0, 10)}...`,
    value: key,
  }));

  const hasRegistries = registryOptions.length > 0;

  return (
    <div className="flex flex-col gap-6">
      <h3 className="text-beige-light text-lg font-semibold">Service Basics</h3>

      {/* Service Name */}
      <div className="flex flex-col gap-2">
        <label className="text-beige-warm text-sm font-medium">Service Name</label>
        <TextInput
          placeholder="e.g. my-wavs-service"
          value={name}
          onChange={setName}
        />
      </div>

      {/* Service Manager (POA Registry) */}
      <div className="flex flex-col gap-2">
        <label className="text-beige-warm text-sm font-medium">Service Manager</label>

        {hasRegistries ? (
          <>
            <Dropdown
              options={registryOptions}
              value={selectedRegistryKey ?? undefined}
              onChange={setSelectedRegistry}
              placeholder="Select a connected registry..."
              size="md"
            />
            {selectedRegistryKey && (
              <RegistryPreview registry={registries.get(selectedRegistryKey) ?? null} />
            )}
          </>
        ) : (
          <div className="flex flex-col gap-3">
            <p className="text-tan-muted text-sm">
              No service contracts connected. Enter manager details manually, or add a contract on this page first.
            </p>
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-2">
                <label className="text-beige-warm text-sm">Chain Key</label>
                <TextInput
                  placeholder="e.g. evm:ethereum"
                  value={manualChain}
                  onChange={setManualChain}
                />
              </div>
              <div className="flex flex-col gap-2">
                <label className="text-beige-warm text-sm">Address</label>
                <TextInput
                  placeholder="0x..."
                  value={manualAddress}
                  onChange={setManualAddress}
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function RegistryPreview({ registry }: { registry: ConnectedRegistry | null }) {
  if (!registry) return null;

  return (
    <div className="p-3 rounded bg-charcoal-dark border border-charcoal-light text-sm">
      <div className="grid grid-cols-2 gap-2">
        <div>
          <span className="text-tan-muted">Chain: </span>
          <span className="text-beige-warm">{registry.chainKey}</span>
        </div>
        <div>
          <span className="text-tan-muted">Chain ID: </span>
          <span className="text-beige-warm">{registry.chainId}</span>
        </div>
        <div className="col-span-2">
          <span className="text-tan-muted">Address: </span>
          <span className="text-beige-warm font-mono text-xs">{registry.address}</span>
        </div>
        {registry.info?.serviceUri && (
          <div className="col-span-2">
            <span className="text-tan-muted">Current Service URI: </span>
            <span className="text-beige-warm text-xs">{registry.info.serviceUri}</span>
          </div>
        )}
      </div>
    </div>
  );
}
