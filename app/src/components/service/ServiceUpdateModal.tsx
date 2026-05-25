import { useState } from 'react';
import { Button, Toast, Modal } from '../atoms';
import { TextInput, Dropdown, type DropdownOption } from '../atoms';
import { uploadToIpfs, saveServiceToNode, getServices } from '../../tauri/commands';
import { setServiceURI } from '../../utils/evm';
import { getPublicClient, getWalletClient } from '../../hooks/useViemClient';
import { useAppStore } from '../../stores/appStore';
import type { Service, IpfsProvider } from '../../types';
import { getErrorMessage, buildServiceMap } from '../../types';

type UploadTarget = 'dev_endpoint' | 'ipfs_local' | 'ipfs_pinata';

type StepStatus = 'pending' | 'in_progress' | 'done' | 'error';

const UPLOAD_OPTIONS: DropdownOption<UploadTarget>[] = [
  { label: 'Dev Endpoint (local node)', value: 'dev_endpoint' },
  { label: 'IPFS (Local)', value: 'ipfs_local' },
  { label: 'IPFS (Pinata)', value: 'ipfs_pinata' },
];

function StatusBadge({ status }: { status: StepStatus }) {
  const colors: Record<StepStatus, string> = {
    pending: 'text-tan-muted',
    in_progress: 'text-yellow-400',
    done: 'text-green-400',
    error: 'text-red-3',
  };
  const labels: Record<StepStatus, string> = {
    pending: 'Pending',
    in_progress: 'In Progress...',
    done: 'Done',
    error: 'Error',
  };
  return <span className={`text-sm font-medium ${colors[status]}`}>{labels[status]}</span>;
}

export interface ServiceUpdateModalProps {
  /** The updated service definition to upload and set on-chain */
  updatedService: Service;
  /** Human-readable description of what changed (e.g. "Pause service", "Update config") */
  description: string;
  /** RPC URL for the chain (required for EVM setServiceURI) */
  rpcUrl: string;
  /** Chain ID (required for EVM setServiceURI) */
  chainId: number;
  /** Contract address of the service manager */
  contractAddress: string;
}

export function ServiceUpdateModal({
  updatedService,
  description,
  rpcUrl,
  chainId,
  contractAddress,
}: ServiceUpdateModalProps) {
  const [uploadTarget, setUploadTarget] = useState<UploadTarget>('dev_endpoint');
  const [localIpfsUrl, setLocalIpfsUrl] = useState('http://127.0.0.1:5001');
  const [pinataApiKey, setPinataApiKey] = useState('');
  const [updating, setUpdating] = useState(false);
  const [uploadStatus, setUploadStatus] = useState<StepStatus>('pending');
  const [setUriStatus, setSetUriStatus] = useState<StepStatus>('pending');
  const [error, setError] = useState<string | null>(null);
  const [resultUri, setResultUri] = useState<string | null>(null);

  const setServices = useAppStore((s) => s.setServices);

  const isEvm = 'evm' in updatedService.manager;
  // TODO: Cosmos setServiceURI support
  const isCosmos = 'cosmos' in updatedService.manager;

  const handleUpdate = async () => {
    setUpdating(true);
    setUploadStatus('pending');
    setSetUriStatus('pending');
    setError(null);
    setResultUri(null);

    try {
      // Step 1: Upload service JSON
      setUploadStatus('in_progress');
      const serviceJson = JSON.stringify(updatedService, null, 2);
      let uri: string;

      if (uploadTarget === 'dev_endpoint') {
        uri = await saveServiceToNode(serviceJson);
      } else {
        const provider: IpfsProvider =
          uploadTarget === 'ipfs_pinata'
            ? { pinata: { api_key: pinataApiKey } }
            : { local: { api_url: localIpfsUrl } };
        const cid = await uploadToIpfs(serviceJson, provider);
        uri = `ipfs://${cid}`;
      }

      setUploadStatus('done');
      setResultUri(uri);

      // Step 2: Set Service URI on-chain
      if (isEvm && rpcUrl && chainId) {
        setSetUriStatus('in_progress');
        const publicClient = getPublicClient(rpcUrl, chainId);
        const walletClient = await getWalletClient(rpcUrl, chainId);
        await setServiceURI(
          publicClient,
          walletClient,
          contractAddress as `0x${string}`,
          uri,
        );
        setSetUriStatus('done');
      } else if (isCosmos) {
        // TODO: Implement Cosmos setServiceURI
        setSetUriStatus('done');
      } else {
        setSetUriStatus('done');
      }

      // Refresh services from the node
      const servicesData = await getServices();
      setServices(await buildServiceMap(servicesData));

      Toast.info(`${description}: service URI updated successfully`);
      Modal.close();
    } catch (err) {
      const msg = getErrorMessage(err);
      setError(msg);

      if (uploadStatus === 'in_progress') {
        setUploadStatus('error');
      } else if (setUriStatus === 'in_progress') {
        setSetUriStatus('error');
      }

      Toast.error(`${description} failed: ${msg}`);
    } finally {
      setUpdating(false);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <h3 className="text-beige-light text-lg font-semibold">{description}</h3>

      {/* Upload Target Config */}
      <div className="p-4 rounded bg-charcoal-medium border border-charcoal-light flex flex-col gap-4">
        <h4 className="text-beige-warm text-sm font-medium">Upload Target</h4>
        <Dropdown
          options={UPLOAD_OPTIONS}
          value={uploadTarget}
          onChange={setUploadTarget}
          size="sm"
        />
        {uploadTarget === 'ipfs_local' && (
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Local IPFS API URL</label>
            <TextInput placeholder="http://127.0.0.1:5001" value={localIpfsUrl} onChange={setLocalIpfsUrl} />
          </div>
        )}
        {uploadTarget === 'ipfs_pinata' && (
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Pinata API Key</label>
            <TextInput kind="password" placeholder="Your Pinata API key" value={pinataApiKey} onChange={setPinataApiKey} />
          </div>
        )}
      </div>

      {isCosmos && (
        <div className="p-3 rounded bg-charcoal-dark border border-yellow-700 text-yellow-300 text-sm">
          Cosmos setServiceURI is not yet supported. The service JSON will be uploaded but the URI will not be set on-chain automatically.
        </div>
      )}

      {/* Progress */}
      <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-medium border border-charcoal-light">
        <h4 className="text-beige-warm text-sm font-medium">Progress</h4>

        <div className="flex items-center justify-between py-2 border-b border-charcoal-light">
          <span className="text-beige-warm text-sm">1. Upload service definition</span>
          <StatusBadge status={uploadStatus} />
        </div>
        {resultUri && (
          <div className="text-xs text-tan-muted font-mono pl-4 break-all">URI: {resultUri}</div>
        )}

        <div className="flex items-center justify-between py-2">
          <span className="text-beige-warm text-sm">2. Set Service URI on-chain</span>
          <StatusBadge status={setUriStatus} />
        </div>

        {error && (
          <div className="p-3 rounded bg-charcoal-dark border border-red-800 text-red-3 text-sm">
            {error}
          </div>
        )}
      </div>

      {/* Action Buttons */}
      <div className="flex gap-3">
        <Button
          text={updating ? 'Updating...' : 'Confirm'}
          color="purple"
          size="lg"
          onClick={handleUpdate}
          disabled={updating}
        />
        <Button
          text="Cancel"
          variant="outline"
          size="lg"
          onClick={() => Modal.close()}
          disabled={updating}
        />
      </div>
    </div>
  );
}
