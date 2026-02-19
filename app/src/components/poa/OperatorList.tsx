import { useState, useEffect } from 'react';
import { type Address, isAddress } from 'viem';
import { AddressDisplay, Button, Toast, TextInput } from '../atoms';
import { usePOAStore, getRegistryKey } from '../../stores/poaStore';
import { useWalletStore } from '../../stores/walletStore';
import { getPublicClient, getWalletClient } from '../../hooks/useViemClient';
import { deregisterOperator, updateOperatorWeight, updateSigningKey, createSigningKeySignature, fetchOperators, registerOperator } from '../../utils/evm';

const ZERO_ADDRESS = '0x0000000000000000000000000000000000000000';

interface OperatorListProps {
  registryKey?: string;
}

export function OperatorList({ registryKey: registryKeyProp }: OperatorListProps) {
  const { getActiveRegistry, updateRegistryOperators, registries } = usePOAStore();
  const registry = registryKeyProp ? registries.get(registryKeyProp) ?? null : getActiveRegistry();
  const { hasMnemonic, derivedAddresses, loadAddresses } = useWalletStore();
  const [loading, setLoading] = useState(false);
  const [editingWeight, setEditingWeight] = useState<Address | null>(null);
  const [newWeight, setNewWeight] = useState('');

  useEffect(() => {
    if (hasMnemonic) {
      loadAddresses();
    }
  }, [hasMnemonic, loadAddresses]);

  if (!registry) {
    return null;
  }

  const { operators, isOwner, address: registryAddress, rpcUrl, chainId } = registry;

  // Map derived addresses to their index for wallet client creation
  const addressIndexMap = new Map<string, number>();
  derivedAddresses.forEach((addr, i) => {
    addressIndexMap.set(addr.toLowerCase(), i);
  });

  const handleDeregister = async (operatorAddress: Address) => {
    if (!confirm(`Are you sure you want to deregister ${operatorAddress}?`)) {
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);

      await deregisterOperator(publicClient, walletClient, registryAddress, operatorAddress);

      // Refresh operators
      const updatedOperators = await fetchOperators(publicClient, registryAddress);
      updateRegistryOperators(getRegistryKey(chainId, registryAddress), updatedOperators);

      Toast.info(`Operator ${operatorAddress} deregistered`);
    } catch (err) {
      console.error('Failed to deregister operator:', err);
      Toast.error(`Failed to deregister operator: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleUpdateWeight = async (operatorAddress: Address) => {
    const weight = BigInt(newWeight);
    if (weight <= 0n) {
      Toast.error('Weight must be greater than 0');
      return;
    }

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      const walletClient = await getWalletClient(rpcUrl, chainId);

      await updateOperatorWeight(publicClient, walletClient, registryAddress, operatorAddress, weight);

      // Refresh operators
      const updatedOperators = await fetchOperators(publicClient, registryAddress);
      updateRegistryOperators(getRegistryKey(chainId, registryAddress), updatedOperators);

      setEditingWeight(null);
      setNewWeight('');
      Toast.info('Operator weight updated');
    } catch (err) {
      console.error('Failed to update weight:', err);
      Toast.error(`Failed to update weight: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSetSigningKey = async (operatorAddress: Address) => {
    const addressIndex = addressIndexMap.get(operatorAddress.toLowerCase());
    if (addressIndex === undefined) return;

    setLoading(true);
    try {
      const publicClient = getPublicClient(rpcUrl, chainId);
      // Create wallet client as the operator (not the owner)
      const operatorWalletClient = await getWalletClient(rpcUrl, chainId, addressIndex);

      // Operator must have ETH to pay for gas
      const balance = await publicClient.getBalance({ address: operatorAddress });
      if (balance === 0n) {
        Toast.error(
          `Operator ${operatorAddress} has no ETH to pay for gas. Fund this address before setting a signing key.`
        );
        return;
      }

      // Use the operator address itself as the signing key
      const signature = await createSigningKeySignature(
        operatorWalletClient.account,
        operatorAddress
      );

      await updateSigningKey(
        publicClient,
        operatorWalletClient,
        registryAddress,
        operatorAddress,
        signature
      );

      // Refresh operators
      const updatedOperators = await fetchOperators(publicClient, registryAddress);
      updateRegistryOperators(getRegistryKey(chainId, registryAddress), updatedOperators);

      Toast.info('Signing key set successfully');
    } catch (err) {
      console.error('Failed to set signing key:', err);
      Toast.error(`Failed to set signing key: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="p-6 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h3 className="text-lg font-semibold text-beige-light mb-4">
        Operators ({operators.length})
      </h3>

      {operators.length === 0 ? (
        <OperatorEmptyState
          isOwner={isOwner}
          registryAddress={registryAddress}
          rpcUrl={rpcUrl}
          chainId={chainId}
        />
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-charcoal-light">
                <th className="text-left py-2 px-3 text-tan-muted text-sm font-medium">
                  Address
                </th>
                <th className="text-left py-2 px-3 text-tan-muted text-sm font-medium">
                  Weight
                </th>
                <th className="text-left py-2 px-3 text-tan-muted text-sm font-medium">
                  Signing Key
                </th>
                {isOwner && (
                  <th className="text-right py-2 px-3 text-tan-muted text-sm font-medium">
                    Actions
                  </th>
                )}
              </tr>
            </thead>
            <tbody>
              {operators.map((operator) => (
                <tr key={operator.address} className="border-b border-charcoal-dark">
                  <td className="py-3 px-3">
                    <AddressDisplay address={operator.address} full />
                  </td>
                  <td className="py-3 px-3">
                    {editingWeight === operator.address ? (
                      <div className="flex items-center gap-2">
                        <input
                          type="number"
                          value={newWeight}
                          onChange={(e) => setNewWeight(e.target.value)}
                          className="w-24 px-2 py-1 text-sm rounded bg-charcoal-dark border border-charcoal-light text-beige-warm"
                          placeholder="Weight"
                        />
                        <button
                          onClick={() => handleUpdateWeight(operator.address)}
                          disabled={loading}
                          className="text-xs text-purple-2 hover:text-purple-3"
                        >
                          Save
                        </button>
                        <button
                          onClick={() => {
                            setEditingWeight(null);
                            setNewWeight('');
                          }}
                          className="text-xs text-tan-muted hover:text-beige-warm"
                        >
                          Cancel
                        </button>
                      </div>
                    ) : (
                      <span className="text-beige-warm text-sm">
                        {operator.weight.toString()}
                      </span>
                    )}
                  </td>
                  <td className="py-3 px-3">
                    <div className="flex items-center gap-2">
                      {operator.signingKey === ZERO_ADDRESS ? (
                        <span className="text-tan-muted text-sm italic">(not set)</span>
                      ) : (
                        <AddressDisplay address={operator.signingKey} full />
                      )}
                      {addressIndexMap.has(operator.address.toLowerCase()) && (
                        <Button
                          text={operator.signingKey === ZERO_ADDRESS ? 'Set' : 'Update'}
                          size="sm"
                          color="purple"
                          variant="outline"
                          disabled={loading}
                          onClick={() => handleSetSigningKey(operator.address)}
                        />
                      )}
                    </div>
                  </td>
                  {isOwner && (
                    <td className="py-3 px-3 text-right">
                      <div className="flex justify-end gap-2">
                        <Button
                          text="Edit"
                          size="sm"
                          variant="outline"
                          disabled={loading || editingWeight !== null}
                          onClick={() => {
                            setEditingWeight(operator.address);
                            setNewWeight(operator.weight.toString());
                          }}
                        />
                        <Button
                          text="Remove"
                          size="sm"
                          color="red"
                          variant="outline"
                          disabled={loading}
                          onClick={() => handleDeregister(operator.address)}
                        />
                      </div>
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function OperatorEmptyState({
  isOwner,
  registryAddress,
  rpcUrl,
  chainId,
}: {
  isOwner: boolean;
  registryAddress: Address;
  rpcUrl: string;
  chainId: number;
}) {
  const { hasMnemonic, derivedAddresses, loadAddresses } = useWalletStore();
  const { updateRegistryOperators } = usePOAStore();
  const [operatorAddress, setOperatorAddress] = useState('');
  const [operatorWeight, setOperatorWeight] = useState('1');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (hasMnemonic) loadAddresses();
  }, [hasMnemonic, loadAddresses]);

  if (!isOwner) {
    return (
      <p className="text-tan-muted italic">
        No operators registered. The contract owner can register operators.
      </p>
    );
  }

  const handleRegister = async () => {
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

      const updatedOperators = await fetchOperators(publicClient, registryAddress, undefined, [operatorAddress as Address]);
      updateRegistryOperators(getRegistryKey(chainId, registryAddress), updatedOperators);

      setOperatorAddress('');
      setOperatorWeight('1');
      Toast.info('Operator registered successfully');
    } catch (err) {
      Toast.error(`Failed to register operator: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <p className="text-tan-muted">No operators registered yet.</p>
      <p className="text-beige-warm text-sm">Register your first operator:</p>
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
      <Button
        text="Register Operator"
        color="purple"
        size="sm"
        disabled={loading}
        onClick={handleRegister}
      />
    </div>
  );
}
