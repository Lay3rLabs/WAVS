import { useState, useEffect } from 'react';
import { formatEther, type Address } from 'viem';
import { mainnet, sepolia, holesky } from 'viem/chains';
import { AddressDisplay, Button } from '../../components/atoms';
import { useWalletStore } from '../../stores/walletStore';
import { getPublicClient } from '../../hooks/useViemClient';
import { getChainConfigs } from '../../tauri';

const KNOWN_CHAIN_NAMES: Record<number, string> = {
  [mainnet.id]: mainnet.name,
  [sepolia.id]: sepolia.name,
  [holesky.id]: holesky.name,
};

function isNumericKey(key: string): boolean {
  return /^\d+$/.test(key);
}

interface ChainBalance {
  chainId: number;
  name: string;
  balance: bigint | null;
  loading: boolean;
  noEndpoint: boolean;
}

function BalanceRow({ chain }: { chain: ChainBalance }) {
  return (
    <div className="flex items-center justify-between py-1.5 px-2 rounded bg-charcoal-darkest">
      <span className="text-tan-muted text-xs">{chain.name}</span>
      <span className="text-beige-warm text-xs font-mono">
        {chain.noEndpoint ? (
          <span className="text-charcoal-light">&mdash;</span>
        ) : chain.loading ? (
          <span className="inline-block w-16 h-3 rounded bg-charcoal-medium animate-pulse" />
        ) : chain.balance !== null ? (
          `${parseFloat(formatEther(chain.balance)).toFixed(4)} ETH`
        ) : (
          <span className="text-red-3 text-xs">error</span>
        )}
      </span>
    </div>
  );
}

export function WalletSection() {
  const {
    hasMnemonic,
    isLoading,
    error: walletError,
    derivedAddresses,
    getMnemonic,
    deleteMnemonic,
    loadAddresses,
    clearError,
  } = useWalletStore();

  const [showMnemonic, setShowMnemonic] = useState(false);
  const [exportedMnemonic, setExportedMnemonic] = useState<string | null>(null);
  const [mnemonicCopied, setMnemonicCopied] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [balances, setBalances] = useState<ChainBalance[][]>([]);

  useEffect(() => {
    if (hasMnemonic) {
      loadAddresses();
    }
  }, [hasMnemonic, loadAddresses]);

  // Fetch balances once addresses are loaded
  useEffect(() => {
    if (derivedAddresses.length === 0) return;

    const fetchBalances = async () => {
      let chains: { chainId: number; name: string; rpcUrl: string | null }[] = [];

      try {
        const configs = await getChainConfigs();

        if (configs.evm) {
          for (const [key, config] of Object.entries(configs.evm)) {
            const chainId = isNumericKey(key) ? parseInt(key, 10) : null;
            if (chainId == null) continue;
            chains.push({
              chainId,
              name: KNOWN_CHAIN_NAMES[chainId] ?? `Chain ${chainId}`,
              rpcUrl: config.http_endpoint ?? null,
            });
          }
        }

        if (configs.dev) {
          for (const [, config] of Object.entries(configs.dev)) {
            if (config.type === 'evm') {
              const chainId = isNumericKey(config.chain_id)
                ? parseInt(config.chain_id, 10)
                : null;
              if (chainId == null) continue;
              chains.push({
                chainId,
                name: KNOWN_CHAIN_NAMES[chainId] ?? `Chain ${chainId}`,
                rpcUrl: config.http_endpoint ?? null,
              });
            }
          }
        }
      } catch {
        // No chain config -- balances will show "--"
      }

      const initialBalances: ChainBalance[][] = derivedAddresses.map(() =>
        chains.map((c) => ({
          chainId: c.chainId,
          name: c.name,
          balance: null,
          loading: c.rpcUrl != null,
          noEndpoint: c.rpcUrl == null,
        }))
      );
      setBalances(initialBalances);

      for (let addrIdx = 0; addrIdx < derivedAddresses.length; addrIdx++) {
        const address = derivedAddresses[addrIdx] as Address;
        for (let chainIdx = 0; chainIdx < chains.length; chainIdx++) {
          const chain = chains[chainIdx];
          if (!chain.rpcUrl) continue;

          getPublicClient(chain.rpcUrl, chain.chainId)
            .getBalance({ address })
            .then((balance) => {
              setBalances((prev) => {
                const next = prev.map((row) => [...row]);
                if (next[addrIdx]?.[chainIdx]) {
                  next[addrIdx][chainIdx] = { ...next[addrIdx][chainIdx], balance, loading: false };
                }
                return next;
              });
            })
            .catch(() => {
              setBalances((prev) => {
                const next = prev.map((row) => [...row]);
                if (next[addrIdx]?.[chainIdx]) {
                  next[addrIdx][chainIdx] = { ...next[addrIdx][chainIdx], balance: null, loading: false };
                }
                return next;
              });
            });
        }
      }
    };

    fetchBalances();
  }, [derivedAddresses]);

  const handleExportWallet = async () => {
    setError(null);
    clearError();
    try {
      const mnemonic = await getMnemonic();
      setExportedMnemonic(mnemonic);
      setShowMnemonic(true);
    } catch {
      setError('Failed to export wallet. Please try again.');
    }
  };

  const handleHideMnemonic = () => {
    setShowMnemonic(false);
    setExportedMnemonic(null);
    setMnemonicCopied(false);
  };

  const handleCopyMnemonic = async () => {
    if (!exportedMnemonic) return;
    await navigator.clipboard.writeText(exportedMnemonic);
    setMnemonicCopied(true);
    setTimeout(() => setMnemonicCopied(false), 2000);
  };

  const handleResetWallet = async () => {
    setError(null);
    clearError();
    try {
      await deleteMnemonic();
      setShowResetConfirm(false);
    } catch {
      setError('Failed to reset wallet. Please try again.');
    }
  };

  const displayError = error || walletError;

  return (
    <div id="wallet" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
      <h2 className="text-beige-light text-lg font-semibold">Wallet</h2>

      {/* Accounts with balances */}
      {hasMnemonic && derivedAddresses.length > 0 && (
        <div className="flex flex-col gap-3">
          {derivedAddresses.map((addr, i) => (
            <div key={i} className="flex flex-col gap-2 p-3 rounded bg-charcoal-dark">
              <div className="flex items-center gap-2">
                <span className="text-tan-muted text-xs w-20 shrink-0">Account {i}</span>
                <AddressDisplay address={addr} full />
              </div>
              {balances[i] && balances[i].length > 0 && (
                <div className="flex flex-col gap-1 ml-[5.5rem]">
                  {balances[i].map((chain) => (
                    <BalanceRow key={chain.chainId} chain={chain} />
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Export/Backup */}
      {hasMnemonic && !showMnemonic && (
        <Button
          text={isLoading ? 'Loading...' : 'Export Recovery Phrase'}
          variant="outline"
          onClick={handleExportWallet}
          disabled={isLoading}
        />
      )}

      {/* Show mnemonic */}
      {showMnemonic && exportedMnemonic && (
        <div className="flex flex-col gap-3">
          <div className="p-3 rounded bg-charcoal-darkest border border-charcoal-light">
            <p className="text-sm text-red-4 mb-2">
              Keep this recovery phrase safe. Anyone with it can access your wallet.
            </p>
            <div className="grid grid-cols-4 gap-2">
              {exportedMnemonic.split(' ').map((word, i) => (
                <div
                  key={i}
                  className="flex items-center gap-1 p-1 rounded bg-charcoal-medium"
                >
                  <span className="text-tan-muted text-xs w-4">{i + 1}.</span>
                  <span className="text-beige-warm font-mono text-xs">
                    {word}
                  </span>
                </div>
              ))}
            </div>
          </div>
          <div className="flex gap-2">
            <Button
              text={mnemonicCopied ? 'Copied!' : 'Copy Recovery Phrase'}
              variant="outline"
              onClick={handleCopyMnemonic}
            />
            <Button text="Hide" variant="outline" onClick={handleHideMnemonic} />
          </div>
        </div>
      )}

      {/* Reset Wallet */}
      {hasMnemonic && !showResetConfirm && (
        <Button
          text="Reset Wallet"
          color="red"
          variant="outline"
          onClick={() => setShowResetConfirm(true)}
        />
      )}

      {/* Reset confirmation */}
      {showResetConfirm && (
        <div className="flex flex-col gap-3 p-3 rounded bg-charcoal-darkest border border-red-2">
          <p className="text-sm text-red-4">
            Are you sure you want to reset your wallet? This will delete your recovery phrase from the keychain.
            Make sure you have backed it up first!
          </p>
          <div className="flex gap-3">
            <Button
              text="Keep Wallet"
              variant="outline"
              onClick={() => setShowResetConfirm(false)}
            />
            <Button
              text={isLoading ? 'Resetting...' : 'Yes, Reset Wallet'}
              color="red"
              onClick={handleResetWallet}
              disabled={isLoading}
            />
          </div>
        </div>
      )}

      {/* Error display */}
      {displayError && (
        <p className="text-red-4 text-sm">{displayError}</p>
      )}
    </div>
  );
}
