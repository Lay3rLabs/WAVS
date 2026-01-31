import { useState, useEffect } from 'react';
import { Button, Dropdown, Expander, Modal, type DropdownOption } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { getChainConfigs, getServices, addService as addServiceCmd } from '../tauri';
import type { ChainKey, Service, ServiceManager } from '../types';
import { getServiceId } from '../types';

export function Services() {
  const services = useAppStore((state) => state.services);
  const setServices = useAppStore((state) => state.setServices);

  const [addressInput, setAddressInput] = useState('');
  const [chainInput, setChainInput] = useState<ChainKey | null>(null);
  const [chains, setChains] = useState<ChainKey[]>([]);
  const [loading, setLoading] = useState(true);

  // Load services and chain configs on mount
  useEffect(() => {
    const init = async () => {
      try {
        const [servicesData, chainConfigs] = await Promise.all([
          getServices(),
          getChainConfigs(),
        ]);

        setServices(servicesData);

        // Extract chain keys from config (cosmos, evm, and dev)
        const chainKeys: ChainKey[] = [
          ...Object.keys(chainConfigs.cosmos || {}).map(k => `cosmos:${k}`),
          ...Object.keys(chainConfigs.evm || {}).map(k => `evm:${k}`),
          ...Object.keys(chainConfigs.dev || {}),
        ];
        setChains(chainKeys);
      } catch (err) {
        console.error('Failed to load services:', err);
        Modal.openError(`Failed to load services: ${err}`);
      } finally {
        setLoading(false);
      }
    };

    init();
  }, [setServices]);

  const handleAddService = async () => {
    if (!chainInput) {
      Modal.openError('Please select a chain.');
      return;
    }
    if (!addressInput.trim()) {
      Modal.openError('Please enter an address.');
      return;
    }

    const address = addressInput.trim();
    const chain = chainInput;

    // Clear inputs
    setAddressInput('');
    setChainInput(null);

    Modal.openInfo(`Adding service for address ${address} on chain ${chain}...`);

    try {
      // Determine if it's EVM or Cosmos based on address format
      // Simple heuristic: 0x prefix = EVM
      const manager: ServiceManager = address.startsWith('0x')
        ? { Evm: { chain, address } }
        : { Cosmos: { chain, address } };

      await addServiceCmd(manager);

      // Reload services
      const servicesData = await getServices();
      setServices(servicesData);

      Modal.openInfo(`Service added for address ${address} on chain ${chain}!`);
    } catch (err) {
      console.error('Failed to add service:', err);
      Modal.openError(`Failed to add service: ${err}`);
    }
  };

  const chainOptions: DropdownOption<ChainKey>[] = chains.map((chain) => ({
    label: chain,
    value: chain,
  }));

  if (loading) {
    return (
      <div className="text-lg text-beige-warm">Loading services...</div>
    );
  }

  const serviceList = Array.from(services.values());

  return (
    <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      <div className="flex flex-col gap-8">
        {/* Add Service Section */}
        <div>
          <h2 className="text-beige-light text-xl font-semibold mb-4">Add Service</h2>
          <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
            <div className="grid grid-cols-2 gap-4 mb-4">
              {/* Address input */}
              <div className="flex flex-col gap-2">
                <label className="text-beige-warm text-sm font-medium">Address</label>
                <input
                  type="text"
                  placeholder="e.g. 0xabc123..."
                  value={addressInput}
                  onChange={(e) => setAddressInput(e.target.value)}
                  className="px-4 py-3 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm text-sm outline-none transition-colors focus:border-tan-muted"
                />
              </div>

              {/* Chain dropdown */}
              <div className="flex flex-col gap-2">
                <label className="text-beige-warm text-sm font-medium">Chain</label>
                <Dropdown
                  options={chainOptions}
                  value={chainInput ?? undefined}
                  onChange={setChainInput}
                  placeholder="Select chain..."
                  size="md"
                />
              </div>
            </div>

            <Button
              text="Add Service"
              color="purple"
              onClick={handleAddService}
            />
          </div>
        </div>

        {/* Active Services List */}
        <div>
          <h2 className="text-beige-light text-xl font-semibold mb-4">Active Services</h2>
          {serviceList.length === 0 ? (
            <div className="p-8 text-center text-tan-muted italic">
              No services configured yet
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
    </div>
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
