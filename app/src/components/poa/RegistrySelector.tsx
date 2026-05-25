import { useState, useEffect } from 'react';
import { type Address, isAddress } from 'viem';
import { Button, Dropdown, TextInput, Toast, type DropdownOption } from '../atoms';
import { usePOAStore, persistRegistries, getRegistryKey } from '../../stores/poaStore';
import { useAppStore } from '../../stores/appStore';
import { getPublicClient, getWalletClient, getAddress } from '../../hooks/useViemClient';
import { getChainConfigs } from '../../tauri';
import { addService as addServiceCmd, getServices } from '../../tauri/commands';
import {
  deployPOARegistry,
  connectToRegistry,
  fetchOperators,
} from '../../utils/evm';
import type { ChainConfigs, ServiceManager } from '../../types';
import { buildServiceMap } from '../../types';

type Mode = 'select' | 'deploy' | 'connect';

interface EvmChainOption {
  key: string;
  chainId: number;
  rpcUrl: string;
}

function isNumericKey(key: string): boolean {
  return /^\d+$/.test(key);
}

interface RegistrySelectorProps {
  onComplete?: () => void;
  onRegistryAdded?: (key: string) => void;
}

export function RegistrySelector({ onComplete, onRegistryAdded }: RegistrySelectorProps = {}) {
  const [mode, setMode] = useState<Mode>('select');
  const [, setChainConfigs] = useState<ChainConfigs | null>(null);
  const [evmChains, setEvmChains] = useState<EvmChainOption[]>([]);
  const [selectedChain, setSelectedChain] = useState<EvmChainOption | null>(null);
  const [loading, setLoading] = useState(true);

  // Deploy form state
  const [thresholdWeight, setThresholdWeight] = useState('1');
  const [quorumNumerator, setQuorumNumerator] = useState('2');
  const [quorumDenominator, setQuorumDenominator] = useState('3');

  // Connect form state
  const [addressInput, setAddressInput] = useState('');

  const { addRegistry, setDeployProgress, deployProgress } = usePOAStore();
  const setServices = useAppStore((s) => s.setServices);

  // Auto-register state
  const [pendingAutoRegister, setPendingAutoRegister] = useState(false);
  const [autoRegisterKey, setAutoRegisterKey] = useState<string | null>(null);
  const [autoRegistering, setAutoRegistering] = useState(false);

  // Load chain configs on mount
  useEffect(() => {
    const init = async () => {
      try {
        const configs = await getChainConfigs();
        setChainConfigs(configs);
        console.log('Chain configs loaded:', JSON.stringify(configs, null, 2));

        // Extract EVM chains from both configs.evm (builders) and configs.dev (full configs)
        const chains: EvmChainOption[] = [];

        // Check configs.evm for builder configs
        if (configs.evm) {
          for (const [key, config] of Object.entries(configs.evm)) {
            console.log(`Checking evm config [${key}]:`, config);
            // Chain ID comes from the key (e.g., "31337")
            const chainId = isNumericKey(key) ? parseInt(key, 10) : null;
            const rpcUrl = config.http_endpoint;

            if (chainId != null && rpcUrl) {
              chains.push({
                key: `evm:${key}`,
                chainId,
                rpcUrl,
              });
            }
          }
        }

        // Check configs.dev for fully-configured EVM chains
        // AnyChainConfig uses internal tagging: { type: 'evm', chain_id: '...', ... }
        if (configs.dev) {
          for (const [key, config] of Object.entries(configs.dev)) {
            console.log(`Checking dev config [${key}]:`, config);
            if (config.type === 'evm') {
              const chainId = isNumericKey(config.chain_id) ? parseInt(config.chain_id, 10) : null;
              const rpcUrl = config.http_endpoint;
              if (chainId != null && rpcUrl) {
                chains.push({
                  key: key,
                  chainId,
                  rpcUrl,
                });
              }
            }
          }
        }

        console.log('Extracted EVM chains:', chains);
        setEvmChains(chains);

        if (chains.length === 0) {
          console.warn('No EVM chains found in config:', configs);
        }
      } catch (err) {
        console.error('Failed to load chain configs:', err);
        Toast.error(`Failed to load chain configs: ${err}`);
      } finally {
        setLoading(false);
      }
    };
    init();
  }, []);

  const chainOptions: DropdownOption<string>[] = evmChains.map((chain) => ({
    label: `${chain.key} (Chain ID: ${chain.chainId})`,
    value: chain.key,
  }));

  const handleChainSelect = (key: string) => {
    const chain = evmChains.find((c) => c.key === key);
    setSelectedChain(chain ?? null);
  };

  const handleDeploy = async () => {
    if (!selectedChain) {
      Toast.error('Please select a chain');
      return;
    }

    const threshold = BigInt(thresholdWeight);
    const qNum = BigInt(quorumNumerator);
    const qDen = BigInt(quorumDenominator);

    if (threshold <= 0n) {
      Toast.error('Threshold weight must be greater than 0');
      return;
    }
    if (qDen <= 0n || qNum > qDen) {
      Toast.error('Invalid quorum: numerator must be <= denominator, denominator must be > 0');
      return;
    }

    setDeployProgress('Preparing deployment...');

    try {
      const publicClient = getPublicClient(selectedChain.rpcUrl, selectedChain.chainId);
      const walletClient = await getWalletClient(selectedChain.rpcUrl, selectedChain.chainId);
      const userAddress = await getAddress();

      const result = await deployPOARegistry(
        publicClient,
        walletClient,
        threshold,
        qNum,
        qDen,
        (step) => setDeployProgress(step)
      );

      setDeployProgress('Fetching registry info...');

      // Connect to the newly deployed registry
      const info = await connectToRegistry(publicClient, result.proxyAddress);
      const operators = await fetchOperators(publicClient, result.proxyAddress);

      addRegistry({
        chainId: selectedChain.chainId,
        chainKey: selectedChain.key,
        rpcUrl: selectedChain.rpcUrl,
        address: result.proxyAddress,
        info,
        operators,
        isOwner: info.owner.toLowerCase() === userAddress.toLowerCase(),
      });

      await persistRegistries();
      const key = getRegistryKey(selectedChain.chainId, result.proxyAddress);
      onRegistryAdded?.(key);
      setDeployProgress(null);
      Toast.info(`Registry deployed at ${result.proxyAddress}`);
      setMode('select');
      onComplete?.();
    } catch (err) {
      console.error('Deployment failed:', err);
      setDeployProgress(null);
      Toast.error(`Deployment failed: ${err}`);
    }
  };

  const handleConnect = async () => {
    if (!selectedChain) {
      Toast.error('Please select a chain');
      return;
    }

    if (!isAddress(addressInput)) {
      Toast.error('Please enter a valid address');
      return;
    }

    const address = addressInput as Address;

    try {
      setDeployProgress('Connecting to registry...');
      const publicClient = getPublicClient(selectedChain.rpcUrl, selectedChain.chainId);
      const userAddress = await getAddress();

      const info = await connectToRegistry(publicClient, address);
      const operators = await fetchOperators(publicClient, address);

      addRegistry({
        chainId: selectedChain.chainId,
        chainKey: selectedChain.key,
        rpcUrl: selectedChain.rpcUrl,
        address,
        info,
        operators,
        isOwner: info.owner.toLowerCase() === userAddress.toLowerCase(),
      });

      await persistRegistries();
      const key = getRegistryKey(selectedChain.chainId, address);
      setDeployProgress(null);

      // Check if registry has a service URI set -- prompt to auto-register
      if (info.serviceUri) {
        setAutoRegisterKey(key);
        setPendingAutoRegister(true);
        setMode('select');
        setAddressInput('');
      } else {
        onRegistryAdded?.(key);
        Toast.info(`Connected to registry at ${address}`);
        setMode('select');
        setAddressInput('');
        onComplete?.();
      }
    } catch (err) {
      console.error('Failed to connect:', err);
      setDeployProgress(null);
      Toast.error(`Failed to connect to registry: ${err}`);
    }
  };

  if (loading) {
    return <div className="text-beige-warm">Loading chain configurations...</div>;
  }

  const handleAutoRegister = async () => {
    if (!autoRegisterKey) return;
    const reg = usePOAStore.getState().registries.get(autoRegisterKey);
    if (!reg) return;

    setAutoRegistering(true);
    try {
      const isEvm = !reg.chainKey.startsWith('cosmos:');
      const manager: ServiceManager = isEvm
        ? { evm: { chain: reg.chainKey, address: reg.address } }
        : { cosmos: { chain: reg.chainKey, address: reg.address } };

      await addServiceCmd(manager);
      const servicesData = await getServices();
      setServices(await buildServiceMap(servicesData));

      setPendingAutoRegister(false);
      onRegistryAdded?.(autoRegisterKey);
      setAutoRegisterKey(null);
      Toast.info('Service registered with WAVS!');
      onComplete?.();
    } catch (err) {
      Toast.error(`Failed to register service: ${err}`);
    } finally {
      setAutoRegistering(false);
    }
  };

  const handleSkipAutoRegister = () => {
    setPendingAutoRegister(false);
    if (autoRegisterKey) {
      onRegistryAdded?.(autoRegisterKey);
    }
    setAutoRegisterKey(null);
    onComplete?.();
  };

  if (mode === 'select') {
    return (
      <div className="flex flex-col gap-6">
        <h2 className="text-xl font-semibold text-beige-light">Service Contracts</h2>
        <p className="text-tan-muted">
          Deploy a new service contract or connect to an existing one.
        </p>
        <div className="flex gap-4">
          <Button
            text="Deploy New Contract"
            color="purple"
            onClick={() => setMode('deploy')}
          />
          <Button
            text="Connect to Existing"
            color="primary"
            onClick={() => setMode('connect')}
          />
        </div>

        {/* Auto-register prompt */}
        {pendingAutoRegister && (
          <div className="p-4 rounded-lg bg-charcoal-medium border border-purple-1">
            <p className="text-beige-warm text-sm mb-3">
              This contract has a Service URI set. Register with WAVS?
            </p>
            <div className="flex gap-3">
              <Button
                text={autoRegistering ? 'Registering...' : 'Register'}
                color="purple"
                size="sm"
                onClick={handleAutoRegister}
                disabled={autoRegistering}
              />
              <Button
                text="Skip"
                size="sm"
                variant="outline"
                onClick={handleSkipAutoRegister}
                disabled={autoRegistering}
              />
            </div>
          </div>
        )}
      </div>
    );
  }

  if (mode === 'deploy') {
    return (
      <div className="flex flex-col gap-6">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold text-beige-light">Deploy New Contract</h2>
          <Button text="Back" size="sm" onClick={() => setMode('select')} />
        </div>

        <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
          <div className="flex flex-col gap-4">
            {/* Chain Selection */}
            <div className="flex flex-col gap-2">
              <label className="text-beige-warm text-sm font-medium">Chain</label>
              {chainOptions.length === 0 ? (
                <p className="text-red-4 text-sm">
                  No EVM chains configured. Please add EVM chain configurations in your WAVS settings.
                </p>
              ) : (
                <Dropdown
                  options={chainOptions}
                  value={selectedChain?.key}
                  onChange={handleChainSelect}
                  placeholder="Select EVM chain..."
                />
              )}
            </div>

            {/* Threshold Weight */}
            <div className="flex flex-col gap-2">
              <label className="text-beige-warm text-sm font-medium">Threshold Weight</label>
              <TextInput
                kind="number"
                placeholder="e.g., 1"
                value={thresholdWeight}
                onChange={setThresholdWeight}
              />
              <p className="text-tan-muted text-xs">
                Minimum total weight of signatures required to validate messages
              </p>
            </div>

            {/* Quorum */}
            <div className="grid grid-cols-2 gap-4">
              <div className="flex flex-col gap-2">
                <label className="text-beige-warm text-sm font-medium">Quorum Numerator</label>
                <TextInput
                  kind="number"
                  placeholder="e.g., 2"
                  value={quorumNumerator}
                  onChange={setQuorumNumerator}
                />
              </div>
              <div className="flex flex-col gap-2">
                <label className="text-beige-warm text-sm font-medium">Quorum Denominator</label>
                <TextInput
                  kind="number"
                  placeholder="e.g., 3"
                  value={quorumDenominator}
                  onChange={setQuorumDenominator}
                />
              </div>
            </div>
            <p className="text-tan-muted text-xs">
              Quorum fraction (e.g., 2/3 means 66.67% of total weight must sign)
            </p>

            {deployProgress && (
              <div className="p-3 rounded bg-charcoal-dark text-beige-warm text-sm">
                {deployProgress}
              </div>
            )}

            <Button
              text="Deploy Contract"
              color="purple"
              onClick={handleDeploy}
              disabled={!selectedChain || !!deployProgress}
            />
          </div>
        </div>
      </div>
    );
  }

  // mode === 'connect'
  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-beige-light">Connect to Contract</h2>
        <Button text="Back" size="sm" onClick={() => setMode('select')} />
      </div>

      <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <div className="flex flex-col gap-4">
          {/* Chain Selection */}
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm font-medium">Chain</label>
            {chainOptions.length === 0 ? (
              <p className="text-red-4 text-sm">
                No EVM chains configured. Please add EVM chain configurations in your WAVS settings.
              </p>
            ) : (
              <Dropdown
                options={chainOptions}
                value={selectedChain?.key}
                onChange={handleChainSelect}
                placeholder="Select EVM chain..."
              />
            )}
          </div>

          {/* Address Input */}
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm font-medium">Registry Address</label>
            <TextInput
              placeholder="0x..."
              value={addressInput}
              onChange={setAddressInput}
            />
          </div>

          {deployProgress && (
            <div className="p-3 rounded bg-charcoal-dark text-beige-warm text-sm">
              {deployProgress}
            </div>
          )}

          <Button
            text="Connect"
            color="purple"
            onClick={handleConnect}
            disabled={!selectedChain || !addressInput || !!deployProgress}
          />
        </div>
      </div>
    </div>
  );
}
