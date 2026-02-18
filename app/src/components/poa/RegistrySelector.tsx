import { useState, useEffect } from 'react';
import { type Address, isAddress } from 'viem';
import { Button, Dropdown, TextInput, Modal, type DropdownOption } from '../atoms';
import { usePOAStore, persistRegistries } from '../../stores/poaStore';
import { getPublicClient, getWalletClient, getAddress } from '../../hooks/useViemClient';
import { getChainConfigs } from '../../tauri';
import {
  deployPOARegistry,
  connectToRegistry,
  fetchOperators,
} from '../../utils/evm';
import type { ChainConfigs } from '../../types';

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
}

export function RegistrySelector({ onComplete }: RegistrySelectorProps = {}) {
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
            // Handle both possible field names: http_endpoint (actual) or rpc_endpoint (typed)
            const configAny = config as unknown as Record<string, unknown>;
            const rpcUrl = (configAny.http_endpoint as string | undefined) ?? config.rpc_endpoint;

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
        if (configs.dev) {
          for (const [key, config] of Object.entries(configs.dev)) {
            console.log(`Checking dev config [${key}]:`, config);
            if ('Evm' in config && config.Evm) {
              chains.push({
                key: key,
                chainId: config.Evm.chain_id,
                rpcUrl: config.Evm.rpc_endpoint,
              });
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
        Modal.openError(`Failed to load chain configs: ${err}`);
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
      Modal.openError('Please select a chain');
      return;
    }

    const threshold = BigInt(thresholdWeight);
    const qNum = BigInt(quorumNumerator);
    const qDen = BigInt(quorumDenominator);

    if (threshold <= 0n) {
      Modal.openError('Threshold weight must be greater than 0');
      return;
    }
    if (qDen <= 0n || qNum > qDen) {
      Modal.openError('Invalid quorum: numerator must be <= denominator, denominator must be > 0');
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
      setDeployProgress(null);
      Modal.openInfo(`Registry deployed at ${result.proxyAddress}`);
      setMode('select');
      onComplete?.();
    } catch (err) {
      console.error('Deployment failed:', err);
      setDeployProgress(null);
      Modal.openError(`Deployment failed: ${err}`);
    }
  };

  const handleConnect = async () => {
    if (!selectedChain) {
      Modal.openError('Please select a chain');
      return;
    }

    if (!isAddress(addressInput)) {
      Modal.openError('Please enter a valid address');
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
      setDeployProgress(null);
      Modal.openInfo(`Connected to registry at ${address}`);
      setMode('select');
      setAddressInput('');
      onComplete?.();
    } catch (err) {
      console.error('Failed to connect:', err);
      setDeployProgress(null);
      Modal.openError(`Failed to connect to registry: ${err}`);
    }
  };

  if (loading) {
    return <div className="text-beige-warm">Loading chain configurations...</div>;
  }

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
