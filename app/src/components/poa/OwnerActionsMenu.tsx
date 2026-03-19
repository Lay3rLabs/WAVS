import { useState, useEffect } from 'react';
import { type Address, isAddress } from 'viem';
import { Button, TextInput, Modal, DropdownMenu, Toast, type MenuOption } from '../atoms';
import { usePOAStore, getRegistryKey } from '../../stores/poaStore';
import { useWalletStore } from '../../stores/walletStore';
import { getPublicClient, getWalletClient } from '../../hooks/useViemClient';
import {
  registerOperator,
  setServiceURI,
  updateStakeThreshold,
  updateQuorum,
  transferOwnership,
  connectToRegistry,
  fetchOperators,
} from '../../utils/evm';

interface OwnerActionsMenuProps {
  registryKey: string;
}

export function OwnerActionsMenu({ registryKey }: OwnerActionsMenuProps) {
  const { registries, updateRegistryInfo, updateRegistryOperators, updateRegistryOwnership } =
    usePOAStore();
  const registry = registries.get(registryKey) ?? null;

  if (!registry || !registry.isOwner) return null;

  const { address: registryAddress, rpcUrl, chainId } = registry;
  const key = getRegistryKey(chainId, registryAddress);

  const refreshRegistry = async (knownAddresses?: Address[]) => {
    const publicClient = getPublicClient(rpcUrl, chainId);
    const [info, operators] = await Promise.all([
      connectToRegistry(publicClient, registryAddress),
      fetchOperators(publicClient, registryAddress, undefined, knownAddresses),
    ]);
    updateRegistryInfo(key, info);
    updateRegistryOperators(key, operators);
  };

  const options: MenuOption[] = [
    {
      label: 'Register Operator',
      onClick: () =>
        Modal.open(
          <RegisterOperatorModal
            registryKey={registryKey}
            registryAddress={registryAddress}
            rpcUrl={rpcUrl}
            chainId={chainId}
            onSuccess={refreshRegistry}
          />,
        ),
    },
    {
      label: 'Set Service URI',
      onClick: () =>
        Modal.open(
          <SetServiceURIModal
            registryAddress={registryAddress}
            rpcUrl={rpcUrl}
            chainId={chainId}
            onSuccess={refreshRegistry}
          />,
        ),
    },
    {
      label: 'Update Threshold',
      onClick: () =>
        Modal.open(
          <UpdateThresholdModal
            registryAddress={registryAddress}
            rpcUrl={rpcUrl}
            chainId={chainId}
            onSuccess={refreshRegistry}
          />,
        ),
    },
    {
      label: 'Update Quorum',
      onClick: () =>
        Modal.open(
          <UpdateQuorumModal
            registryAddress={registryAddress}
            rpcUrl={rpcUrl}
            chainId={chainId}
            onSuccess={refreshRegistry}
          />,
        ),
    },
    {
      label: 'Transfer Ownership',
      variant: 'danger',
      onClick: () =>
        Modal.open(
          <TransferOwnershipModal
            registryKey={registryKey}
            registryAddress={registryAddress}
            rpcUrl={rpcUrl}
            chainId={chainId}
            onSuccess={async () => {
              await refreshRegistry();
              updateRegistryOwnership(key, false);
            }}
          />,
        ),
    },
  ];

  return <DropdownMenu label="Owner Actions" options={options} size="sm" />;
}

// --- Modals ---

interface ModalBaseProps {
  registryAddress: Address;
  rpcUrl: string;
  chainId: number;
  onSuccess: (knownAddresses?: Address[]) => Promise<void>;
}

function RegisterOperatorModal({
  registryKey: _registryKey,
  registryAddress,
  rpcUrl,
  chainId,
  onSuccess,
}: ModalBaseProps & { registryKey: string }) {
  const { hasMnemonic, derivedAddresses, loadAddresses } = useWalletStore();
  const [operatorAddress, setOperatorAddress] = useState('');
  const [operatorWeight, setOperatorWeight] = useState('1');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (hasMnemonic) loadAddresses();
  }, [hasMnemonic, loadAddresses]);

  const handleSubmit = async () => {
    if (!isAddress(operatorAddress)) {
      Toast.error('Please enter a valid operator address');
      return;
    }
    const weight = BigInt(operatorWeight);
    if (weight <= 0n) {
      Toast.error('Weight must be greater than 0');
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);
      await registerOperator(publicClient, walletClient, registryAddress, operatorAddress as Address, weight);
      await onSuccess([operatorAddress as Address]);
      Modal.close();
      Toast.info('Operator registered successfully');
    } catch (err) {
      Toast.error(`Failed to register operator: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-6 min-w-[400px]">
      <h3 className="text-lg font-semibold text-beige-light">Register Operator</h3>
      <div className="grid grid-cols-2 gap-3">
        <TextInput placeholder="Operator address (0x...)" value={operatorAddress} onChange={setOperatorAddress} />
        <TextInput kind="number" placeholder="Weight" value={operatorWeight} onChange={setOperatorWeight} />
      </div>
      {derivedAddresses.length > 0 && (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-tan-muted">Use my address:</span>
          {derivedAddresses.map((addr, i) => (
            <button
              key={i}
              onClick={() => setOperatorAddress(addr)}
              className="text-xs px-2 py-1 rounded bg-charcoal-dark hover:bg-charcoal-light text-tan-muted hover:text-beige-warm transition-colors font-mono"
              title={addr}
            >
              Account {i}
            </button>
          ))}
        </div>
      )}
      <div className="flex gap-3 justify-end">
        <Button text="Cancel" size="sm" variant="outline" onClick={() => Modal.close()} disabled={loading} />
        <Button text="Register" color="purple" size="sm" onClick={handleSubmit} disabled={loading} />
      </div>
    </div>
  );
}

function SetServiceURIModal({ registryAddress, rpcUrl, chainId, onSuccess }: ModalBaseProps) {
  const [uri, setUri] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    if (!uri.trim()) {
      Toast.error('Please enter a service URI');
      return;
    }
    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);
      await setServiceURI(publicClient, walletClient, registryAddress, uri);
      await onSuccess();
      Modal.close();
      Toast.info('Service URI updated');
    } catch (err) {
      Toast.error(`Failed to set service URI: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-6 min-w-[400px]">
      <h3 className="text-lg font-semibold text-beige-light">Set Service URI</h3>
      <TextInput placeholder="https://example.com/service or ipfs://..." value={uri} onChange={setUri} />
      <div className="flex gap-3 justify-end">
        <Button text="Cancel" size="sm" variant="outline" onClick={() => Modal.close()} disabled={loading} />
        <Button text="Update" color="purple" size="sm" onClick={handleSubmit} disabled={loading} />
      </div>
    </div>
  );
}

function UpdateThresholdModal({ registryAddress, rpcUrl, chainId, onSuccess }: ModalBaseProps) {
  const [threshold, setThreshold] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    const val = BigInt(threshold);
    if (val <= 0n) {
      Toast.error('Threshold must be greater than 0');
      return;
    }
    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);
      await updateStakeThreshold(publicClient, walletClient, registryAddress, val);
      await onSuccess();
      Modal.close();
      Toast.info('Threshold weight updated');
    } catch (err) {
      Toast.error(`Failed to update threshold: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-6 min-w-[400px]">
      <h3 className="text-lg font-semibold text-beige-light">Update Threshold Weight</h3>
      <TextInput kind="number" placeholder="New threshold weight" value={threshold} onChange={setThreshold} />
      <div className="flex gap-3 justify-end">
        <Button text="Cancel" size="sm" variant="outline" onClick={() => Modal.close()} disabled={loading} />
        <Button text="Update" color="purple" size="sm" onClick={handleSubmit} disabled={loading} />
      </div>
    </div>
  );
}

function UpdateQuorumModal({ registryAddress, rpcUrl, chainId, onSuccess }: ModalBaseProps) {
  const [num, setNum] = useState('');
  const [den, setDen] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    const numVal = BigInt(num);
    const denVal = BigInt(den);
    if (denVal <= 0n) {
      Toast.error('Denominator must be greater than 0');
      return;
    }
    if (numVal > denVal) {
      Toast.error('Numerator cannot exceed denominator');
      return;
    }
    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);
      await updateQuorum(publicClient, walletClient, registryAddress, numVal, denVal);
      await onSuccess();
      Modal.close();
      Toast.info('Quorum updated');
    } catch (err) {
      Toast.error(`Failed to update quorum: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-6 min-w-[400px]">
      <h3 className="text-lg font-semibold text-beige-light">Update Quorum</h3>
      <div className="grid grid-cols-2 gap-3">
        <TextInput kind="number" placeholder="Numerator" value={num} onChange={setNum} />
        <TextInput kind="number" placeholder="Denominator" value={den} onChange={setDen} />
      </div>
      <div className="flex gap-3 justify-end">
        <Button text="Cancel" size="sm" variant="outline" onClick={() => Modal.close()} disabled={loading} />
        <Button text="Update" color="purple" size="sm" onClick={handleSubmit} disabled={loading} />
      </div>
    </div>
  );
}

function TransferOwnershipModal({
  registryKey: _registryKey,
  registryAddress,
  rpcUrl,
  chainId,
  onSuccess,
}: ModalBaseProps & { registryKey: string }) {
  const [newOwner, setNewOwner] = useState('');
  const [confirmed, setConfirmed] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleSubmit = async () => {
    if (!isAddress(newOwner)) {
      Toast.error('Please enter a valid address');
      return;
    }
    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);
      await transferOwnership(publicClient, walletClient, registryAddress, newOwner as Address);
      await onSuccess();
      Modal.close();
      Toast.info('Ownership transferred. You are no longer the owner.');
    } catch (err) {
      Toast.error(`Failed to transfer ownership: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-4 p-6 min-w-[400px]">
      <h3 className="text-lg font-semibold text-red-3">Transfer Ownership</h3>
      {!confirmed ? (
        <>
          <TextInput placeholder="New owner address (0x...)" value={newOwner} onChange={setNewOwner} />
          <p className="text-red-4 text-sm">
            This will transfer ownership of the registry contract. This action cannot be undone.
          </p>
          <div className="flex gap-3 justify-end">
            <Button text="Cancel" size="sm" variant="outline" onClick={() => Modal.close()} />
            <Button text="Continue" color="red" size="sm" onClick={() => setConfirmed(true)} disabled={!newOwner} />
          </div>
        </>
      ) : (
        <>
          <div className="p-3 rounded bg-charcoal-dark border border-red-2">
            <p className="text-red-4 text-sm">
              Are you sure you want to transfer ownership to{' '}
              <span className="font-mono">{newOwner}</span>?
            </p>
          </div>
          <div className="flex gap-3 justify-end">
            <Button text="Go Back" size="sm" variant="outline" onClick={() => setConfirmed(false)} disabled={loading} />
            <Button text="Confirm Transfer" color="red" size="sm" onClick={handleSubmit} disabled={loading} />
          </div>
        </>
      )}
    </div>
  );
}
