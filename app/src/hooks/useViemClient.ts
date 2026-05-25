import {
  createPublicClient,
  createWalletClient,
  http,
  type Chain,
  type PublicClient,
  type WalletClient,
  type Transport,
  type Address,
} from 'viem';
import { mnemonicToAccount, type HDAccount } from 'viem/accounts';
import { useWalletStore } from '../stores/walletStore';
import { mainnet, sepolia, holesky } from 'viem/chains';

/**
 * Known chains for easy lookup
 */
const KNOWN_CHAINS: Record<number, Chain> = {
  1: mainnet,
  11155111: sepolia,
  17000: holesky,
};

/**
 * Create a custom chain definition from chainId and rpcUrl
 */
function createCustomChain(chainId: number, rpcUrl: string, name?: string): Chain {
  // Check if it's a known chain
  if (KNOWN_CHAINS[chainId]) {
    return KNOWN_CHAINS[chainId];
  }

  // Create a minimal custom chain
  return {
    id: chainId,
    name: name || `Chain ${chainId}`,
    nativeCurrency: {
      name: 'Ether',
      symbol: 'ETH',
      decimals: 18,
    },
    rpcUrls: {
      default: { http: [rpcUrl] },
    },
  };
}

/**
 * Get a read-only public client for interacting with the blockchain
 */
export function getPublicClient(rpcUrl: string, chainId: number): PublicClient<Transport, Chain> {
  const chain = createCustomChain(chainId, rpcUrl);

  return createPublicClient({
    chain,
    transport: http(rpcUrl),
  });
}

/**
 * Get a wallet client for signing transactions
 * Uses the mnemonic from the OS keychain
 */
export async function getWalletClient(
  rpcUrl: string,
  chainId: number,
  addressIndex: number = 0
): Promise<WalletClient<Transport, Chain, HDAccount>> {
  const chain = createCustomChain(chainId, rpcUrl);
  const { getMnemonic } = useWalletStore.getState();
  const mnemonic = await getMnemonic();

  const account = mnemonicToAccount(mnemonic, { addressIndex });

  return createWalletClient({
    account,
    chain,
    transport: http(rpcUrl),
  });
}

/**
 * Get the derived address at a specific index
 * Uses the mnemonic from the OS keychain
 */
export async function getAddress(addressIndex: number = 0): Promise<Address> {
  const { getMnemonic } = useWalletStore.getState();
  const mnemonic = await getMnemonic();

  const account = mnemonicToAccount(mnemonic, { addressIndex });
  return account.address;
}

/**
 * Hook wrapper for React components
 */
export function useViemClient() {
  return {
    getPublicClient,
    getWalletClient,
    getAddress,
  };
}
