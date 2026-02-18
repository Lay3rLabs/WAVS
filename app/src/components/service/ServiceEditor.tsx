import { useState, useMemo } from 'react';
import { Button, Modal, TextInput, Dropdown, type DropdownOption } from '../atoms';
import { usePOAStore } from '../../stores/poaStore';
import { useAppStore } from '../../stores/appStore';
import { uploadToIpfs, addService as addServiceCmd, removeService as removeServiceCmd, getServices } from '../../tauri/commands';
import { setServiceURI } from '../../utils/evm';
import { getPublicClient, getWalletClient } from '../../hooks/useViemClient';
import type { Service, ServiceManager, IpfsProvider } from '../../types';
import { getErrorMessage } from '../../types';

type IpfsProviderType = 'local' | 'pinata';
type DeployStepStatus = 'pending' | 'in_progress' | 'done' | 'error';

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

interface ServiceEditorProps {
  service: Service;
  registryKey: string;
  onClose: () => void;
}

export function ServiceEditor({ service, registryKey, onClose }: ServiceEditorProps) {
  const [jsonText, setJsonText] = useState(() => JSON.stringify(service, null, 2));
  const [ipfsProvider, setIpfsProvider] = useState<IpfsProviderType>('local');
  const [localIpfsUrl, setLocalIpfsUrl] = useState('http://127.0.0.1:5001');
  const [pinataApiKey, setPinataApiKey] = useState('');
  const [deploying, setDeploying] = useState(false);
  const [deployStatus, setDeployStatus] = useState({
    ipfs: 'pending' as DeployStepStatus,
    setUri: 'pending' as DeployStepStatus,
    remove: 'pending' as DeployStepStatus,
    register: 'pending' as DeployStepStatus,
    cid: null as string | null,
    error: null as string | null,
  });

  const registries = usePOAStore((s) => s.registries);
  const registry = registries.get(registryKey) ?? null;
  const setServices = useAppStore((s) => s.setServices);

  const jsonValid = useMemo(() => {
    try {
      JSON.parse(jsonText);
      return true;
    } catch {
      return false;
    }
  }, [jsonText]);

  const handleSaveAndRedeploy = async () => {
    if (!jsonValid || !registry) return;

    let parsedService: Service;
    try {
      parsedService = JSON.parse(jsonText) as Service;
    } catch {
      Modal.openError('Invalid JSON');
      return;
    }

    // Ensure manager matches the registry
    const isEvm = !registry.chainKey.startsWith('cosmos:');
    const manager: ServiceManager = isEvm
      ? { evm: { chain: registry.chainKey, address: registry.address } }
      : { cosmos: { chain: registry.chainKey, address: registry.address } };
    parsedService.manager = manager;

    setDeploying(true);
    setDeployStatus({ ipfs: 'pending', setUri: 'pending', remove: 'pending', register: 'pending', cid: null, error: null });

    try {
      // Step 1: Upload to IPFS
      setDeployStatus((s) => ({ ...s, ipfs: 'in_progress' }));
      const content = JSON.stringify(parsedService, null, 2);
      const provider: IpfsProvider = ipfsProvider === 'pinata'
        ? { pinata: { api_key: pinataApiKey } }
        : { local: { api_url: localIpfsUrl } };
      const cid = await uploadToIpfs(content, provider);
      setDeployStatus((s) => ({ ...s, ipfs: 'done', cid }));

      // Step 2: Set Service URI on-chain
      if ('evm' in manager && registry.rpcUrl && registry.chainId) {
        setDeployStatus((s) => ({ ...s, setUri: 'in_progress' }));
        const uri = `ipfs://${cid}`;
        const publicClient = getPublicClient(registry.rpcUrl, registry.chainId);
        const walletClient = await getWalletClient(registry.rpcUrl, registry.chainId);
        await setServiceURI(publicClient, walletClient, registry.address as `0x${string}`, uri);
        setDeployStatus((s) => ({ ...s, setUri: 'done' }));
      } else {
        setDeployStatus((s) => ({ ...s, setUri: 'done' }));
      }

      // Step 3: Remove old service from WAVS
      setDeployStatus((s) => ({ ...s, remove: 'in_progress' }));
      await removeServiceCmd(manager);
      setDeployStatus((s) => ({ ...s, remove: 'done' }));

      // Step 4: Re-add service to WAVS
      setDeployStatus((s) => ({ ...s, register: 'in_progress' }));
      await addServiceCmd(manager);
      const servicesData = await getServices();
      setServices(servicesData);
      setDeployStatus((s) => ({ ...s, register: 'done' }));

      Modal.openInfo('Service updated and redeployed successfully!');
      onClose();
    } catch (err) {
      const msg = getErrorMessage(err);
      setDeployStatus((s) => {
        const updated = { ...s, error: msg };
        if (s.ipfs === 'in_progress') updated.ipfs = 'error';
        else if (s.setUri === 'in_progress') updated.setUri = 'error';
        else if (s.remove === 'in_progress') updated.remove = 'error';
        else if (s.register === 'in_progress') updated.register = 'error';
        return updated;
      });
      Modal.openError(`Redeploy failed: ${msg}`);
    } finally {
      setDeploying(false);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-beige-light text-xl font-semibold">
          Edit Service: {service.name}
        </h2>
        <Button text="Cancel" size="sm" variant="outline" onClick={onClose} disabled={deploying} />
      </div>

      {/* JSON Editor */}
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <label className="text-beige-warm text-sm font-medium">Service JSON</label>
          <span className={`text-xs font-medium ${jsonValid ? 'text-green-400' : 'text-red-3'}`}>
            {jsonValid ? 'Valid JSON' : 'Invalid JSON'}
          </span>
        </div>
        <textarea
          className="w-full h-80 p-3 rounded bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-xs resize-y focus:outline-none focus:border-purple-1"
          value={jsonText}
          onChange={(e) => setJsonText(e.target.value)}
          spellCheck={false}
        />
      </div>

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
        <h4 className="text-beige-warm text-sm font-medium">Redeploy Progress</h4>

        <div className="flex items-center justify-between py-2 border-b border-charcoal-light">
          <span className="text-beige-warm text-sm">1. Upload to IPFS</span>
          <StatusBadge status={deployStatus.ipfs} />
        </div>
        {deployStatus.cid && (
          <div className="text-xs text-tan-muted font-mono pl-4">CID: {deployStatus.cid}</div>
        )}

        <div className="flex items-center justify-between py-2 border-b border-charcoal-light">
          <span className="text-beige-warm text-sm">2. Set Service URI on-chain</span>
          <StatusBadge status={deployStatus.setUri} />
        </div>

        <div className="flex items-center justify-between py-2 border-b border-charcoal-light">
          <span className="text-beige-warm text-sm">3. Remove old service from WAVS</span>
          <StatusBadge status={deployStatus.remove} />
        </div>

        <div className="flex items-center justify-between py-2">
          <span className="text-beige-warm text-sm">4. Re-register with WAVS</span>
          <StatusBadge status={deployStatus.register} />
        </div>

        {deployStatus.error && (
          <div className="p-3 rounded bg-charcoal-dark border border-red-800 text-red-3 text-sm">
            {deployStatus.error}
          </div>
        )}
      </div>

      {/* Save & Redeploy Button */}
      <Button
        text={deploying ? 'Redeploying...' : 'Save & Redeploy'}
        color="purple"
        size="lg"
        onClick={handleSaveAndRedeploy}
        disabled={deploying || !jsonValid}
      />
    </div>
  );
}
