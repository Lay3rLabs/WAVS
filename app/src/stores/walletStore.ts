import { create } from 'zustand';
import { generateMnemonic as generateBip39Mnemonic, validateMnemonic as validateBip39Mnemonic } from '@scure/bip39';
import { wordlist as englishWordlist } from '@scure/bip39/wordlists/english';
import { mnemonicToAccount } from 'viem/accounts';
import {
  hasMnemonic as hasMnemonicCommand,
  storeMnemonic as storeMnemonicCommand,
  getMnemonic as getMnemonicCommand,
  deleteMnemonic as deleteMnemonicCommand,
} from '../tauri';

interface WalletState {
  // State
  hasMnemonic: boolean;
  isLoading: boolean;
  error: string | null;
  derivedAddresses: string[];

  // Setup state (only during setup flow)
  pendingMnemonic: string | null;

  // Actions
  checkMnemonic: () => Promise<boolean>;
  generateMnemonic: () => Promise<void>;
  importMnemonic: (mnemonic: string) => Promise<boolean>;
  saveMnemonic: () => Promise<void>;
  getMnemonic: () => Promise<string>;
  deleteMnemonic: () => Promise<void>;
  loadAddresses: () => Promise<void>;
  clearPendingMnemonic: () => void;

  // Error handling
  setError: (error: string | null) => void;
  clearError: () => void;
}

/**
 * Derives HD addresses from a mnemonic using the standard BIP44 path for Ethereum.
 * Path: m/44'/60'/0'/0/{index}
 */
async function deriveAddresses(mnemonic: string, count: number = 3): Promise<string[]> {
  const addresses: string[] = [];
  for (let i = 0; i < count; i++) {
    const account = mnemonicToAccount(mnemonic, { addressIndex: i });
    addresses.push(account.address);
  }
  return addresses;
}

/**
 * Validates a BIP39 mnemonic phrase.
 */
function validateMnemonic(mnemonic: string): boolean {
  const words = mnemonic.trim().split(/\s+/);
  if (words.length !== 12 && words.length !== 24) {
    return false;
  }
  return validateBip39Mnemonic(mnemonic.trim(), englishWordlist);
}

/**
 * Normalizes a mnemonic by trimming and collapsing whitespace.
 */
function normalizeMnemonic(mnemonic: string): string {
  return mnemonic.trim().split(/\s+/).join(' ');
}

export const useWalletStore = create<WalletState>((set, get) => ({
  // Initial state
  hasMnemonic: false,
  isLoading: false,
  error: null,
  derivedAddresses: [],
  pendingMnemonic: null,

  // Check if mnemonic exists in keychain
  checkMnemonic: async () => {
    set({ isLoading: true, error: null });
    try {
      const has = await hasMnemonicCommand();
      set({ hasMnemonic: has, isLoading: false });
      return has;
    } catch (err) {
      set({
        error: `Failed to check keychain: ${err}`,
        isLoading: false,
      });
      return false;
    }
  },

  // Generate a new 24-word mnemonic
  generateMnemonic: async () => {
    set({ isLoading: true, error: null });
    try {
      const mnemonic = generateBip39Mnemonic(englishWordlist, 256); // 24 words
      const addresses = await deriveAddresses(mnemonic, 3);
      set({
        pendingMnemonic: mnemonic,
        derivedAddresses: addresses,
        isLoading: false,
      });
    } catch (err) {
      set({
        error: `Failed to generate mnemonic: ${err}`,
        isLoading: false,
      });
    }
  },

  // Import and validate an existing mnemonic
  importMnemonic: async (mnemonic: string) => {
    set({ isLoading: true, error: null });
    const normalized = normalizeMnemonic(mnemonic);

    if (!validateMnemonic(normalized)) {
      set({
        error: 'Invalid mnemonic phrase. Please check your words and try again.',
        isLoading: false,
      });
      return false;
    }

    try {
      const addresses = await deriveAddresses(normalized, 3);
      set({
        pendingMnemonic: normalized,
        derivedAddresses: addresses,
        isLoading: false,
      });
      return true;
    } catch (err) {
      set({
        error: `Failed to derive addresses: ${err}`,
        isLoading: false,
      });
      return false;
    }
  },

  // Save pending mnemonic to OS keychain
  saveMnemonic: async () => {
    const { pendingMnemonic } = get();
    if (!pendingMnemonic) {
      set({ error: 'No mnemonic to save' });
      return;
    }

    set({ isLoading: true, error: null });
    try {
      await storeMnemonicCommand(pendingMnemonic);
      set({
        hasMnemonic: true,
        pendingMnemonic: null, // Clear from memory
        isLoading: false,
      });
    } catch (err) {
      set({
        error: `Failed to save mnemonic to keychain: ${err}`,
        isLoading: false,
      });
    }
  },

  // Get mnemonic from OS keychain (for export/backup)
  getMnemonic: async () => {
    set({ isLoading: true, error: null });
    try {
      const mnemonic = await getMnemonicCommand();
      set({ isLoading: false });
      return mnemonic;
    } catch (err) {
      set({
        error: `Failed to retrieve mnemonic from keychain: ${err}`,
        isLoading: false,
      });
      throw err;
    }
  },

  // Delete mnemonic from OS keychain (reset wallet)
  deleteMnemonic: async () => {
    set({ isLoading: true, error: null });
    try {
      await deleteMnemonicCommand();
      set({
        hasMnemonic: false,
        pendingMnemonic: null,
        derivedAddresses: [],
        isLoading: false,
      });
    } catch (err) {
      set({
        error: `Failed to delete mnemonic from keychain: ${err}`,
        isLoading: false,
      });
    }
  },

  // Load derived addresses from stored mnemonic
  loadAddresses: async () => {
    if (get().derivedAddresses.length > 0) return;
    set({ isLoading: true, error: null });
    try {
      const mnemonic = await getMnemonicCommand();
      const addresses = await deriveAddresses(mnemonic, 3);
      set({ derivedAddresses: addresses, isLoading: false });
    } catch (err) {
      set({
        error: `Failed to load addresses: ${err}`,
        isLoading: false,
      });
    }
  },

  // Clear pending mnemonic from memory
  clearPendingMnemonic: () => {
    set({
      pendingMnemonic: null,
      derivedAddresses: [],
    });
  },

  // Error handling
  setError: (error) => set({ error }),
  clearError: () => set({ error: null }),
}));
