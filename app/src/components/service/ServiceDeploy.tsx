import { useMemo } from 'react';
import { Button, Toast } from '../atoms';
import { useServiceBuilderStore, type DeployStepStatus } from '../../stores/serviceBuilderStore';
import { usePOAStore } from '../../stores/poaStore';
import { TextInput, Dropdown, type DropdownOption } from '../atoms';
import { uploadToIpfs } from '../../tauri/commands';
import { addService as addServiceCmd, getServices } from '../../tauri/commands';
import { setServiceURI } from '../../utils/evm';
import { getPublicClient, getWalletClient } from '../../hooks/useViemClient';
import { useAppStore } from '../../stores/appStore';
import type { ServiceManager, IpfsProvider } from '../../types';
import { getErrorMessage, buildServiceMap } from '../../types';

type IpfsProviderType = 'local' | 'pinata';

const IPFS_OPTIONS: DropdownOption<IpfsProviderType>[] = [
  { label: 'Local IPFS', value: 'local' },
  { label: 'Pinata', value: 'pinata' },
];

function StatusBadge({ status }: { status: DeployStepStatus }) {
  const colors: Record<DeployStepStatus, string> = {
    pending: 'text-tan-muted',
    in_progress: 'text-yellow-400',
    done: 'text-green-400',
    error: 'text-red-3',
  };
  const labels: Record<DeployStepStatus, string> = {
    pending: 'Pending',
    in_progress: 'In Progress...',
    done: 'Done',
    error: 'Error',
  };
  return <span className={`text-sm font-medium ${colors[status]}`}>{labels[status]}</span>;
}

interface ServiceDeployProps {
  onDeployComplete?: (chainId: number, address: string) => void;
}

export function ServiceDeploy({ onDeployComplete }: ServiceDeployProps) {
  const buildServiceJson = useServiceBuilderStore((s) => s.buildServiceJson);
  const selectedRegistryKey = useServiceBuilderStore((s) => s.selectedRegistryKey);
  const manualChain = useServiceBuilderStore((s) => s.manualChain);
  const manualAddress = useServiceBuilderStore((s) => s.manualAddress);

  const ipfsProvider = useServiceBuilderStore((s) => s.ipfsProvider);
  const localIpfsUrl = useServiceBuilderStore((s) => s.localIpfsUrl);
  const pinataApiKey = useServiceBuilderStore((s) => s.pinataApiKey);
  const setIpfsProvider = useServiceBuilderStore((s) => s.setIpfsProvider);
  const setLocalIpfsUrl = useServiceBuilderStore((s) => s.setLocalIpfsUrl);
  const setPinataApiKey = useServiceBuilderStore((s) => s.setPinataApiKey);

  const deploying = useServiceBuilderStore((s) => s.deploying);
  const setDeploying = useServiceBuilderStore((s) => s.setDeploying);
  const deployState = useServiceBuilderStore((s) => s.deployState);
  const setDeployState = useServiceBuilderStore((s) => s.setDeployState);

  const registries = usePOAStore((s) => s.registries);
  const setServices = useAppStore((s) => s.setServices);

  // Resolve the service manager
  const resolvedManager: { manager: ServiceManager; chainKey: string; chainId: number; rpcUrl: string; address: string } | null = useMemo(() => {
    if (selectedRegistryKey) {
      const registry = registries.get(selectedRegistryKey);
      if (registry) {
        const isEvm = !registry.chainKey.startsWith('cosmos:');
        const manager: ServiceManager = isEvm
          ? { evm: { chain: registry.chainKey, address: registry.address } }
          : { cosmos: { chain: registry.chainKey, address: registry.address } };
        return {
          manager,
          chainKey: registry.chainKey,
          chainId: registry.chainId,
          rpcUrl: registry.rpcUrl,
          address: registry.address,
        };
      }
    }
    if (manualChain && manualAddress) {
      const manager: ServiceManager = manualAddress.startsWith('0x')
        ? { evm: { chain: manualChain, address: manualAddress } }
        : { cosmos: { chain: manualChain, address: manualAddress } };
      return { manager, chainKey: manualChain, chainId: 0, rpcUrl: '', address: manualAddress };
    }
    return null;
  }, [selectedRegistryKey, registries, manualChain, manualAddress]);

  const service = useMemo(() => {
    const built = buildServiceJson();
    if (!built || !resolvedManager) return null;
    return { ...built, manager: resolvedManager.manager };
  }, [buildServiceJson, resolvedManager]);

  const handleDeploy = async () => {
    if (!service || !resolvedManager) {
      Toast.error('Service or manager is not configured properly.');
      return;
    }

    setDeploying(true);
    setDeployState({ ipfsStatus: 'pending', setUriStatus: 'pending', registerStatus: 'pending', cid: null, error: null });

    try {
      // Step 1: Upload to IPFS
      setDeployState({ ipfsStatus: 'in_progress' });
      const content = JSON.stringify(service, null, 2);
      const provider: IpfsProvider = ipfsProvider === 'pinata'
        ? { pinata: { api_key: pinataApiKey } }
        : { local: { api_url: localIpfsUrl } };

      const cid = await uploadToIpfs(content, provider);
      setDeployState({ ipfsStatus: 'done', cid });

      // Step 2: Set Service URI on-chain (EVM only)
      if ('evm' in resolvedManager.manager && resolvedManager.rpcUrl && resolvedManager.chainId) {
        setDeployState({ setUriStatus: 'in_progress' });
        const uri = `ipfs://${cid}`;
        const publicClient = getPublicClient(resolvedManager.rpcUrl, resolvedManager.chainId);
        const walletClient = await getWalletClient(resolvedManager.rpcUrl, resolvedManager.chainId);
        await setServiceURI(publicClient, walletClient, resolvedManager.address as `0x${string}`, uri);
        setDeployState({ setUriStatus: 'done' });
      } else {
        // Skip for cosmos or when no RPC available
        setDeployState({ setUriStatus: 'done' });
      }

      // Step 3: Register with WAVS
      setDeployState({ registerStatus: 'in_progress' });
      await addServiceCmd(resolvedManager.manager);
      const servicesData = await getServices();
      setServices(await buildServiceMap(servicesData));
      setDeployState({ registerStatus: 'done' });

      if (onDeployComplete && resolvedManager) {
        onDeployComplete(resolvedManager.chainId, resolvedManager.address);
      } else {
        Toast.info('Service deployed successfully!');
      }
    } catch (err) {
      const msg = getErrorMessage(err);
      setDeployState({ error: msg });

      // Mark current step as error
      if (deployState.ipfsStatus === 'in_progress') {
        setDeployState({ ipfsStatus: 'error' });
      } else if (deployState.setUriStatus === 'in_progress') {
        setDeployState({ setUriStatus: 'error' });
      } else if (deployState.registerStatus === 'in_progress') {
        setDeployState({ registerStatus: 'error' });
      }

      Toast.error(`Deploy failed: ${msg}`);
    } finally {
      setDeploying(false);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <h3 className="text-beige-light text-lg font-semibold">Deploy Service</h3>

      {/* IPFS Provider Config */}
      <div className="p-4 rounded bg-charcoal-medium border border-charcoal-light flex flex-col gap-4">
        <h4 className="text-beige-warm text-sm font-medium">IPFS Upload</h4>
        <Dropdown
          options={IPFS_OPTIONS}
          value={ipfsProvider}
          onChange={setIpfsProvider}
          size="sm"
        />
        {ipfsProvider === 'local' && (
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Local IPFS API URL</label>
            <TextInput placeholder="http://127.0.0.1:5001" value={localIpfsUrl} onChange={setLocalIpfsUrl} />
          </div>
        )}
        {ipfsProvider === 'pinata' && (
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Pinata API Key</label>
            <TextInput kind="password" placeholder="Your Pinata API key" value={pinataApiKey} onChange={setPinataApiKey} />
          </div>
        )}
      </div>

      {/* Deploy Progress */}
      <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-medium border border-charcoal-light">
        <h4 className="text-beige-warm text-sm font-medium">Deploy Progress</h4>

        <div className="flex items-center justify-between py-2 border-b border-charcoal-light">
          <span className="text-beige-warm text-sm">1. Upload to IPFS</span>
          <StatusBadge status={deployState.ipfsStatus} />
        </div>
        {deployState.cid && (
          <div className="text-xs text-tan-muted font-mono pl-4">CID: {deployState.cid}</div>
        )}

        <div className="flex items-center justify-between py-2 border-b border-charcoal-light">
          <span className="text-beige-warm text-sm">2. Set Service URI on-chain</span>
          <StatusBadge status={deployState.setUriStatus} />
        </div>

        <div className="flex items-center justify-between py-2">
          <span className="text-beige-warm text-sm">3. Register with WAVS</span>
          <StatusBadge status={deployState.registerStatus} />
        </div>

        {deployState.error && (
          <div className="p-3 rounded bg-charcoal-dark border border-red-800 text-red-3 text-sm">
            {deployState.error}
          </div>
        )}
      </div>

      {/* Deploy Button */}
      <Button
        text={deploying ? 'Deploying...' : 'Deploy Service'}
        color="purple"
        size="lg"
        onClick={handleDeploy}
        disabled={deploying || !service}
      />
    </div>
  );
}
