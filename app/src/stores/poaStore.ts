import { create } from 'zustand';
import type { Address } from 'viem';
import type { RegistryInfo, Operator } from '../contracts/POAStakeRegistry';
import type { SavedRegistry } from '../types';
import { savePoaRegistries } from '../tauri/commands';

export interface ConnectedRegistry {
  chainId: number;
  chainKey: string;
  rpcUrl: string;
  address: Address;
  info: RegistryInfo | null;
  operators: Operator[];
  isOwner: boolean;
}

interface POAState {
  // State
  registries: Map<string, ConnectedRegistry>;
  activeRegistryKey: string | null;
  isLoading: boolean;
  error: string | null;
  deployProgress: string | null;

  // Actions
  setActiveRegistry: (key: string | null) => void;
  addRegistry: (registry: ConnectedRegistry) => void;
  removeRegistry: (key: string) => void;
  updateRegistryInfo: (key: string, info: RegistryInfo) => void;
  updateRegistryOperators: (key: string, operators: Operator[]) => void;
  updateRegistryOwnership: (key: string, isOwner: boolean) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  setDeployProgress: (progress: string | null) => void;
  clearError: () => void;
  clearRegistries: () => void;
  getActiveRegistry: () => ConnectedRegistry | null;
}

/**
 * Generate a unique key for a registry
 */
export function getRegistryKey(chainId: number, address: Address): string {
  return `${chainId}:${address.toLowerCase()}`;
}

export const usePOAStore = create<POAState>((set, get) => ({
  // Initial state
  registries: new Map(),
  activeRegistryKey: null,
  isLoading: false,
  error: null,
  deployProgress: null,

  // Actions
  setActiveRegistry: (key) => set({ activeRegistryKey: key }),

  addRegistry: (registry) => {
    const key = getRegistryKey(registry.chainId, registry.address);
    set((state) => {
      const newRegistries = new Map(state.registries);
      newRegistries.set(key, registry);
      return { registries: newRegistries, activeRegistryKey: key };
    });
  },

  removeRegistry: (key) => {
    set((state) => {
      const newRegistries = new Map(state.registries);
      newRegistries.delete(key);
      const newActiveKey = state.activeRegistryKey === key ? null : state.activeRegistryKey;
      return { registries: newRegistries, activeRegistryKey: newActiveKey };
    });
  },

  updateRegistryInfo: (key, info) => {
    set((state) => {
      const registry = state.registries.get(key);
      if (!registry) return state;
      const newRegistries = new Map(state.registries);
      newRegistries.set(key, { ...registry, info });
      return { registries: newRegistries };
    });
  },

  updateRegistryOperators: (key, operators) => {
    set((state) => {
      const registry = state.registries.get(key);
      if (!registry) return state;
      const newRegistries = new Map(state.registries);
      newRegistries.set(key, { ...registry, operators });
      return { registries: newRegistries };
    });
  },

  updateRegistryOwnership: (key, isOwner) => {
    set((state) => {
      const registry = state.registries.get(key);
      if (!registry) return state;
      const newRegistries = new Map(state.registries);
      newRegistries.set(key, { ...registry, isOwner });
      return { registries: newRegistries };
    });
  },

  setLoading: (loading) => set({ isLoading: loading }),
  setError: (error) => set({ error }),
  setDeployProgress: (progress) => set({ deployProgress: progress }),
  clearError: () => set({ error: null }),
  clearRegistries: () => set({ registries: new Map(), activeRegistryKey: null }),

  getActiveRegistry: () => {
    const state = get();
    if (!state.activeRegistryKey) return null;
    return state.registries.get(state.activeRegistryKey) ?? null;
  },
}));

/**
 * Persist the current registries to settings.json via the backend.
 */
export async function persistRegistries() {
  const registries = usePOAStore.getState().registries;
  const entries: SavedRegistry[] = Array.from(registries.values()).map((r) => ({
    chain_id: r.chainId,
    chain_key: r.chainKey,
    rpc_url: r.rpcUrl,
    address: r.address,
  }));
  await savePoaRegistries(entries);
}
