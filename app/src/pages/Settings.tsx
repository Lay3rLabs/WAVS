import { useState, useEffect, useCallback } from 'react';
import { formatEther, type Address } from 'viem';
import { mainnet, sepolia, holesky } from 'viem/chains';
import { AddressDisplay, Button, TomlEditor } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { useWalletStore } from '../stores/walletStore';
import { setWavsHome, restart, readWavsToml, writeWavsToml } from '../tauri';
import { getPublicClient } from '../hooks/useViemClient';
import { getChainConfigs } from '../tauri';

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
          <span className="text-charcoal-light">—</span>
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

export function Settings() {
  const settings = useAppStore((state) => state.settings);
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

  const [changed, setChanged] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showMnemonic, setShowMnemonic] = useState(false);
  const [exportedMnemonic, setExportedMnemonic] = useState<string | null>(null);
  const [showResetConfirm, setShowResetConfirm] = useState(false);

  // Per-account, per-chain balances: balances[accountIndex][chainIndex]
  const [balances, setBalances] = useState<ChainBalance[][]>([]);

  // TOML editor state
  const [tomlContent, setTomlContent] = useState('');
  const [savedContent, setSavedContent] = useState('');
  const [tomlLoading, setTomlLoading] = useState(false);
  const [tomlError, setTomlError] = useState<string | null>(null);
  const [tomlSaveSuccess, setTomlSaveSuccess] = useState(false);
  const hasUnsavedChanges = tomlContent !== savedContent;

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
        // No chain config — balances will show "—"
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

  const loadToml = useCallback(async () => {
    if (!settings.wavs_home) return;
    setTomlLoading(true);
    setTomlError(null);
    setTomlSaveSuccess(false);
    try {
      const content = await readWavsToml();
      setTomlContent(content);
      setSavedContent(content);
    } catch (err) {
      setTomlError(String(err));
    } finally {
      setTomlLoading(false);
    }
  }, [settings.wavs_home]);

  useEffect(() => {
    loadToml();
  }, [loadToml]);

  const handleSaveToml = async () => {
    setTomlError(null);
    setTomlSaveSuccess(false);
    try {
      await writeWavsToml(tomlContent);
      setSavedContent(tomlContent);
      setTomlSaveSuccess(true);
      setChanged(true);
    } catch (err) {
      setTomlError(String(err));
    }
  };

  const handleReloadToml = async () => {
    await loadToml();
  };

  const handleBrowse = async () => {
    setError(null);
    try {
      const path = await setWavsHome();
      if (path) {
        console.log('Changed wavs_home to', path);
        setChanged(true);
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const handleRestart = async () => {
    try {
      await restart();
    } catch (err) {
      console.error('Failed to restart application:', err);
    }
  };

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
    <div className="flex flex-col gap-6 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {/* Restart warning */}
      {changed && (
        <div className="flex gap-4 mb-4 items-center">
          <div className="flex-1 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
            <p className="text-lg text-beige-light">
              Restart for changes to take effect.
            </p>
          </div>
          <Button
            text="Restart Application"
            color="red"
            onClick={handleRestart}
          />
        </div>
      )}

      {/* Wallet Section */}
      <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
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
            <Button text="Hide" variant="outline" onClick={handleHideMnemonic} />
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
                text="Cancel"
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
      </div>

      {/* WAVS Home Directory */}
      <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h2 className="text-beige-light text-lg font-semibold">
          WAVS Home Directory
        </h2>
        <div className="flex gap-3 items-center">
          <input
            type="text"
            readOnly
            placeholder="No directory selected"
            value={settings.wavs_home ?? ''}
            className="flex-1 px-4 py-3 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
          />
          <Button text="Browse..." onClick={handleBrowse} />
        </div>
      </div>

      {/* TOML Editor */}
      {settings.wavs_home && (
        <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <h2 className="text-beige-light text-lg font-semibold">
                Configuration (wavs.toml)
              </h2>
              {hasUnsavedChanges && (
                <span className="text-tan-muted text-sm italic">
                  (unsaved changes)
                </span>
              )}
            </div>
            <div className="flex gap-2">
              <Button
                text="Reload"
                variant="outline"
                onClick={handleReloadToml}
                disabled={tomlLoading}
              />
              <Button
                text={tomlLoading ? 'Saving...' : 'Save'}
                onClick={handleSaveToml}
                disabled={tomlLoading || !hasUnsavedChanges}
              />
            </div>
          </div>

          {tomlLoading && !tomlContent ? (
            <div className="text-tan-muted text-sm p-4">Loading...</div>
          ) : (
            <TomlEditor
              value={tomlContent}
              onChange={setTomlContent}
              height="60vh"
            />
          )}

          {tomlError && (
            <p className="text-red-4 text-sm">{tomlError}</p>
          )}
          {tomlSaveSuccess && (
            <p className="text-green-4 text-sm">
              Configuration saved successfully.
            </p>
          )}
        </div>
      )}

      {/* Error display */}
      {displayError && (
        <p className="text-red-4 text-base">{displayError}</p>
      )}
    </div>
  );
}
