import { useState } from 'react';
import { type Address, isAddress } from 'viem';
import { Button, TextInput, Modal } from '../atoms';
import { usePOAStore, getRegistryKey } from '../../stores/poaStore';
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

export function OwnerActions() {
  const { getActiveRegistry, updateRegistryInfo, updateRegistryOperators, updateRegistryOwnership } =
    usePOAStore();
  const registry = getActiveRegistry();

  // Form states
  const [loading, setLoading] = useState(false);

  // Register operator form
  const [operatorAddress, setOperatorAddress] = useState('');
  const [operatorWeight, setOperatorWeight] = useState('1');

  // Service URI form
  const [serviceUri, setServiceUri] = useState('');

  // Threshold form
  const [thresholdWeight, setThresholdWeight] = useState('');

  // Quorum form
  const [quorumNum, setQuorumNum] = useState('');
  const [quorumDen, setQuorumDen] = useState('');

  // Transfer ownership form
  const [newOwner, setNewOwner] = useState('');
  const [showTransferConfirm, setShowTransferConfirm] = useState(false);

  if (!registry || !registry.isOwner) {
    return null;
  }

  const { address: registryAddress, rpcUrl, chainId } = registry;
  const key = getRegistryKey(chainId, registryAddress);

  const refreshRegistry = async () => {
    const publicClient = getPublicClient(rpcUrl, chainId);
    const [info, operators] = await Promise.all([
      connectToRegistry(publicClient, registryAddress),
      fetchOperators(publicClient, registryAddress),
    ]);
    updateRegistryInfo(key, info);
    updateRegistryOperators(key, operators);
  };

  const handleRegisterOperator = async () => {
    if (!isAddress(operatorAddress)) {
      Modal.openError('Please enter a valid operator address');
      return;
    }

    const weight = BigInt(operatorWeight);
    if (weight <= 0n) {
      Modal.openError('Weight must be greater than 0');
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);

      await registerOperator(
        publicClient,
        walletClient,
        registryAddress,
        operatorAddress as Address,
        weight
      );

      await refreshRegistry();
      setOperatorAddress('');
      setOperatorWeight('1');
      Modal.openInfo('Operator registered successfully');
    } catch (err) {
      console.error('Failed to register operator:', err);
      Modal.openError(`Failed to register operator: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSetServiceUri = async () => {
    if (!serviceUri.trim()) {
      Modal.openError('Please enter a service URI');
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);

      await setServiceURI(publicClient, walletClient, registryAddress, serviceUri);

      await refreshRegistry();
      setServiceUri('');
      Modal.openInfo('Service URI updated');
    } catch (err) {
      console.error('Failed to set service URI:', err);
      Modal.openError(`Failed to set service URI: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleUpdateThreshold = async () => {
    const threshold = BigInt(thresholdWeight);
    if (threshold <= 0n) {
      Modal.openError('Threshold must be greater than 0');
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);

      await updateStakeThreshold(publicClient, walletClient, registryAddress, threshold);

      await refreshRegistry();
      setThresholdWeight('');
      Modal.openInfo('Threshold weight updated');
    } catch (err) {
      console.error('Failed to update threshold:', err);
      Modal.openError(`Failed to update threshold: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleUpdateQuorum = async () => {
    const num = BigInt(quorumNum);
    const den = BigInt(quorumDen);

    if (den <= 0n) {
      Modal.openError('Quorum denominator must be greater than 0');
      return;
    }
    if (num > den) {
      Modal.openError('Quorum numerator cannot exceed denominator');
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);

      await updateQuorum(publicClient, walletClient, registryAddress, num, den);

      await refreshRegistry();
      setQuorumNum('');
      setQuorumDen('');
      Modal.openInfo('Quorum updated');
    } catch (err) {
      console.error('Failed to update quorum:', err);
      Modal.openError(`Failed to update quorum: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleTransferOwnership = async () => {
    if (!isAddress(newOwner)) {
      Modal.openError('Please enter a valid address');
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);

      await transferOwnership(publicClient, walletClient, registryAddress, newOwner as Address);

      await refreshRegistry();
      updateRegistryOwnership(key, false);
      setNewOwner('');
      setShowTransferConfirm(false);
      Modal.openInfo('Ownership transferred. You are no longer the owner.');
    } catch (err) {
      console.error('Failed to transfer ownership:', err);
      Modal.openError(`Failed to transfer ownership: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <h3 className="text-lg font-semibold text-beige-light">Owner Actions</h3>

      {/* Register Operator */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h4 className="text-sm font-medium text-beige-warm mb-3">Register Operator</h4>
        <div className="flex flex-col gap-3">
          <div className="grid grid-cols-2 gap-3">
            <TextInput
              placeholder="Operator address (0x...)"
              value={operatorAddress}
              onChange={setOperatorAddress}
            />
            <TextInput
              kind="number"
              placeholder="Weight"
              value={operatorWeight}
              onChange={setOperatorWeight}
            />
          </div>
          <Button
            text="Register Operator"
            color="purple"
            size="sm"
            disabled={loading}
            onClick={handleRegisterOperator}
          />
        </div>
      </div>

      {/* Set Service URI */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h4 className="text-sm font-medium text-beige-warm mb-3">Set Service URI</h4>
        <div className="flex flex-col gap-3">
          <TextInput
            placeholder="https://example.com/service"
            value={serviceUri}
            onChange={setServiceUri}
          />
          <Button
            text="Update Service URI"
            color="purple"
            size="sm"
            disabled={loading}
            onClick={handleSetServiceUri}
          />
        </div>
      </div>

      {/* Update Threshold */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h4 className="text-sm font-medium text-beige-warm mb-3">Update Threshold Weight</h4>
        <div className="flex flex-col gap-3">
          <TextInput
            kind="number"
            placeholder="New threshold weight"
            value={thresholdWeight}
            onChange={setThresholdWeight}
          />
          <Button
            text="Update Threshold"
            color="purple"
            size="sm"
            disabled={loading}
            onClick={handleUpdateThreshold}
          />
        </div>
      </div>

      {/* Update Quorum */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h4 className="text-sm font-medium text-beige-warm mb-3">Update Quorum</h4>
        <div className="flex flex-col gap-3">
          <div className="grid grid-cols-2 gap-3">
            <TextInput
              kind="number"
              placeholder="Numerator"
              value={quorumNum}
              onChange={setQuorumNum}
            />
            <TextInput
              kind="number"
              placeholder="Denominator"
              value={quorumDen}
              onChange={setQuorumDen}
            />
          </div>
          <Button
            text="Update Quorum"
            color="purple"
            size="sm"
            disabled={loading}
            onClick={handleUpdateQuorum}
          />
        </div>
      </div>

      {/* Transfer Ownership */}
      <div className="p-4 rounded-lg bg-charcoal-medium border border-red-2">
        <h4 className="text-sm font-medium text-red-3 mb-3">Transfer Ownership</h4>
        {!showTransferConfirm ? (
          <div className="flex flex-col gap-3">
            <TextInput
              placeholder="New owner address (0x...)"
              value={newOwner}
              onChange={setNewOwner}
            />
            <Button
              text="Transfer Ownership"
              color="red"
              size="sm"
              disabled={loading || !newOwner}
              onClick={() => setShowTransferConfirm(true)}
            />
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            <p className="text-red-4 text-sm">
              Are you sure you want to transfer ownership to{' '}
              <span className="font-mono">{newOwner}</span>? This action cannot be undone.
            </p>
            <div className="flex gap-2">
              <Button
                text="Confirm Transfer"
                color="red"
                size="sm"
                disabled={loading}
                onClick={handleTransferOwnership}
              />
              <Button
                text="Cancel"
                size="sm"
                onClick={() => {
                  setShowTransferConfirm(false);
                  setNewOwner('');
                }}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
