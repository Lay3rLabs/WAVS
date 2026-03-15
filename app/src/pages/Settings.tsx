import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { formatEther, type Address } from 'viem';
import { mainnet, sepolia, holesky } from 'viem/chains';
import { AddressDisplay, Button, TomlEditor } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { useWalletStore } from '../stores/walletStore';
import { setWavsHome, restart, readWavsToml, writeWavsToml, startMcpServer, stopMcpServer, getMcpStatus, getMcpBinaryPath, getWavsUrl, saveMcpSettings, clearPersistedServices, registerClaudeMcp, saveEnvVars, pickFolder } from '../tauri';
import { usePOAStore } from '../stores/poaStore';
import { errorMessage } from '../utils/error';
import type { McpStatus } from '../types';
import { getPublicClient } from '../hooks/useViemClient';
import { getChainConfigs } from '../tauri';

const ENV_VAR_SUGGESTIONS = [
  // Open-source / local AI
  { label: 'HuggingFace', key: 'WAVS_ENV_HUGGINGFACE_API_KEY'  },
  { label: 'Ollama URL',  key: 'WAVS_ENV_OLLAMA_BASE_URL'      },
  { label: 'LM Studio',   key: 'WAVS_ENV_LM_STUDIO_BASE_URL'   },
  { label: 'Together AI', key: 'WAVS_ENV_TOGETHER_API_KEY'     },
  { label: 'Groq',        key: 'WAVS_ENV_GROQ_API_KEY'         },
  { label: 'Mistral',     key: 'WAVS_ENV_MISTRAL_API_KEY'      },
  { label: 'Replicate',   key: 'WAVS_ENV_REPLICATE_API_TOKEN'  },
  // Closed-source AI
  { label: 'OpenAI',      key: 'WAVS_ENV_OPENAI_API_KEY'       },
  { label: 'Anthropic',   key: 'WAVS_ENV_ANTHROPIC_API_KEY'    },
  // Decentralized storage
  { label: 'Pinata',      key: 'WAVS_ENV_PINATA_JWT'           },
  { label: 'Web3.Storage', key: 'WAVS_ENV_WEB3_STORAGE_TOKEN' },
  // Blockchain / data
  { label: 'Etherscan',   key: 'WAVS_ENV_ETHERSCAN_API_KEY'    },
  { label: 'Alchemy',     key: 'WAVS_ENV_ALCHEMY_API_KEY'      },
  { label: 'Infura',      key: 'WAVS_ENV_INFURA_API_KEY'       },
  { label: 'The Graph',   key: 'WAVS_ENV_THEGRAPH_API_KEY'     },
  { label: 'CoinGecko',   key: 'WAVS_ENV_COINGECKO_API_KEY'    },
  // General
  { label: 'GitHub',      key: 'WAVS_ENV_GITHUB_TOKEN'         },
];

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

  // MCP server state
  const [mcpStatus, setMcpStatus] = useState<McpStatus | null>(null);
  const [mcpBinaryPath, setMcpBinaryPath] = useState<string | null>(null);
  const [wavsUrl, setWavsUrl] = useState('http://localhost:8000');
  const [mcpAutoStart, setMcpAutoStart] = useState(settings.mcp_auto_start ?? false);
  const [mcpToken, setMcpToken] = useState(settings.mcp_token ?? '');
  const [mcpLoading, setMcpLoading] = useState(false);
  const [mcpError, setMcpError] = useState<string | null>(null);
  const [claudeProjectPath, setClaudeProjectPath] = useState('');
  const [claudeRegisterResult, setClaudeRegisterResult] = useState<string | null>(null);
  const [claudeRegisterLoading, setClaudeRegisterLoading] = useState(false);
  const [claudeRegisterError, setClaudeRegisterError] = useState<string | null>(null);
  const [showMnemonic, setShowMnemonic] = useState(false);
  const [exportedMnemonic, setExportedMnemonic] = useState<string | null>(null);
  const [mnemonicCopied, setMnemonicCopied] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [showClearServicesConfirm, setShowClearServicesConfirm] = useState(false);

  // Per-account, per-chain balances: balances[accountIndex][chainIndex]
  const [balances, setBalances] = useState<ChainBalance[][]>([]);

  // TOML editor state
  const [tomlContent, setTomlContent] = useState('');
  const [savedContent, setSavedContent] = useState('');
  const [tomlLoading, setTomlLoading] = useState(false);
  const [tomlError, setTomlError] = useState<string | null>(null);
  const [tomlSaveSuccess, setTomlSaveSuccess] = useState(false);
  const hasUnsavedChanges = tomlContent !== savedContent;

  // Environment variables state
  const [envVars, setEnvVars] = useState<Record<string, string>>({});
  const [newEnvKey, setNewEnvKey] = useState('');
  const [newEnvValue, setNewEnvValue] = useState('');
  const [visibleEnvKeys, setVisibleEnvKeys] = useState<Set<string>>(new Set());
  const [envSaving, setEnvSaving] = useState(false);
  const [envSaveSuccess, setEnvSaveSuccess] = useState(false);
  const [envError, setEnvError] = useState<string | null>(null);
  const newEnvValueRef = useRef<HTMLInputElement>(null);

  // Collect all env_keys from registered services, not yet set in envVars
  const neededByServices = useMemo(() => {
    const keys = new Set<string>();
    for (const service of settings.saved_services ?? []) {
      for (const workflow of Object.values(service.workflows)) {
        for (const k of workflow.component.env_keys ?? []) keys.add(k);
        if (typeof workflow.submit === 'object' && 'aggregator' in workflow.submit) {
          for (const k of workflow.submit.aggregator.component.env_keys ?? []) keys.add(k);
        }
      }
    }
    return [...keys].filter((k) => !(k in envVars));
  }, [settings.saved_services, envVars]);

  // Static suggestions not yet set
  const staticSuggestions = useMemo(
    () => ENV_VAR_SUGGESTIONS.filter((s) => !(s.key in envVars)),
    [envVars]
  );

  const handleSuggestionClick = (key: string) => {
    setNewEnvKey(key);
    newEnvValueRef.current?.focus();
  };

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

  // Poll MCP status every 3 seconds; also resolve the binary path once
  useEffect(() => {
    getMcpBinaryPath().then(setMcpBinaryPath).catch(() => {});
    getWavsUrl().then(setWavsUrl).catch(() => {});

    let cancelled = false;
    const poll = async () => {
      try {
        const status = await getMcpStatus();
        if (!cancelled) setMcpStatus(status);
      } catch {
        // not fatal
      }
    };
    poll();
    const id = setInterval(poll, 3000);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  const handleMcpToggle = async () => {
    setMcpLoading(true);
    setMcpError(null);
    try {
      if (mcpStatus?.running) {
        await stopMcpServer();
      } else {
        await startMcpServer();
      }
      setMcpStatus(await getMcpStatus());
    } catch (e) {
      setMcpError(errorMessage(e));
    } finally {
      setMcpLoading(false);
    }
  };

  const handleMcpSaveSettings = async () => {
    setMcpError(null);
    try {
      await saveMcpSettings(mcpAutoStart, mcpToken.trim() || null);
    } catch (e) {
      setMcpError(errorMessage(e));
    }
  };

  const handleRegisterClaude = async () => {
    setClaudeRegisterLoading(true);
    setClaudeRegisterError(null);
    setClaudeRegisterResult(null);
    try {
      const result = await registerClaudeMcp(claudeProjectPath.trim());
      setClaudeRegisterResult(result);
    } catch (e) {
      setClaudeRegisterError(errorMessage(e));
    } finally {
      setClaudeRegisterLoading(false);
    }
  };

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

  // Sync env vars from settings store on load
  useEffect(() => {
    setEnvVars(settings.env_vars ?? {});
  }, [settings.env_vars]);

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

  const handleAddEnvVar = () => {
    let key = newEnvKey.trim();
    if (!key) return;
    if (!key.startsWith('WAVS_ENV_')) {
      key = `WAVS_ENV_${key}`;
    }
    setEnvVars((prev) => ({ ...prev, [key]: newEnvValue }));
    setNewEnvKey('');
    setNewEnvValue('');
  };

  const handleRemoveEnvVar = (key: string) => {
    setEnvVars((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
    setVisibleEnvKeys((prev) => {
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  };

  const handleToggleEnvVisibility = (key: string) => {
    setVisibleEnvKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const handleSaveEnvVars = async () => {
    setEnvSaving(true);
    setEnvError(null);
    setEnvSaveSuccess(false);
    try {
      await saveEnvVars(envVars);
      setEnvSaveSuccess(true);
    } catch (e) {
      setEnvError(errorMessage(e));
    } finally {
      setEnvSaving(false);
    }
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

  const handleClearServices = async () => {
    setError(null);
    try {
      await clearPersistedServices();
      usePOAStore.getState().clearRegistries();
      setShowClearServicesConfirm(false);
    } catch {
      setError('Failed to clear app state. Please try again.');
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

      {/* Environment Variables */}
      <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h2 className="text-beige-light text-lg font-semibold">Environment Variables</h2>
        <p className="text-tan-muted text-xs">
          <span className="font-mono">WAVS_ENV_*</span> variables are passed to workflow components that declare them in their <span className="font-mono">env_keys</span> list.
        </p>

        {/* Required by services */}
        {neededByServices.length > 0 && (
          <div className="flex flex-col gap-1.5">
            <span className="text-tan-muted text-xs font-medium">Required by your services</span>
            <div className="flex flex-wrap gap-1.5">
              {neededByServices.map((key) => (
                <button
                  key={key}
                  className="px-2 py-0.5 rounded text-xs font-mono bg-charcoal-dark border border-charcoal-light text-tan-muted hover:text-beige-warm hover:border-tan-muted transition-colors"
                  title={key}
                  onClick={() => handleSuggestionClick(key)}
                >
                  {key}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Common integrations */}
        {staticSuggestions.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-tan-muted text-xs">Suggestions:</span>
            {staticSuggestions.map((s) => (
              <button
                key={s.key}
                className="px-2 py-0.5 rounded text-xs font-mono bg-charcoal-dark border border-charcoal-light text-tan-muted hover:text-beige-warm hover:border-tan-muted transition-colors"
                title={s.key}
                onClick={() => handleSuggestionClick(s.key)}
              >
                {s.label}
              </button>
            ))}
          </div>
        )}

        {/* Existing vars */}
        {Object.keys(envVars).length > 0 && (
          <div className="flex flex-col gap-2">
            {Object.entries(envVars).map(([key, value]) => (
              <div key={key} className="flex items-center gap-2">
                <span className="text-beige-warm font-mono text-xs w-48 shrink-0 truncate" title={key}>{key}</span>
                <input
                  type={visibleEnvKeys.has(key) ? 'text' : 'password'}
                  readOnly
                  value={value}
                  className="flex-1 px-3 py-1.5 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-xs outline-none"
                />
                <Button
                  text={visibleEnvKeys.has(key) ? 'Hide' : 'Show'}
                  variant="outline"
                  onClick={() => handleToggleEnvVisibility(key)}
                />
                <Button
                  text="Remove"
                  color="red"
                  variant="outline"
                  onClick={() => handleRemoveEnvVar(key)}
                />
              </div>
            ))}
          </div>
        )}

        {/* Add new var */}
        <div className="flex items-center gap-2">
          <input
            type="text"
            placeholder="Key (WAVS_ENV_ prefix added if missing)"
            value={newEnvKey}
            onChange={(e) => setNewEnvKey(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') handleAddEnvVar(); }}
            className="flex-1 px-3 py-1.5 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-xs outline-none"
          />
          <input
            ref={newEnvValueRef}
            type="text"
            placeholder="Value"
            value={newEnvValue}
            onChange={(e) => setNewEnvValue(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') handleAddEnvVar(); }}
            className="flex-1 px-3 py-1.5 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-xs outline-none"
          />
          <Button
            text="Add"
            variant="outline"
            onClick={handleAddEnvVar}
            disabled={!newEnvKey.trim()}
          />
        </div>

        <div className="flex items-center justify-between">
          <div>
            {envSaveSuccess && (
              <p className="text-green-4 text-sm">Environment variables saved.</p>
            )}
            {envError && (
              <p className="text-red-4 text-sm">{envError}</p>
            )}
          </div>
          <Button
            text={envSaving ? 'Saving...' : 'Save'}
            onClick={handleSaveEnvVars}
            disabled={envSaving}
          />
        </div>
      </div>

      {/* MCP Server */}
      <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-beige-light text-lg font-semibold">MCP Server</h2>
            {mcpStatus && (
              <span className={`text-xs font-mono px-2 py-0.5 rounded ${
                mcpStatus.running
                  ? 'bg-charcoal-dark text-green-4'
                  : 'bg-charcoal-dark text-tan-muted'
              }`}>
                {mcpStatus.running ? `Running (pid ${mcpStatus.pid})` : 'Stopped'}
              </span>
            )}
          </div>
          <Button
            text={mcpLoading ? '...' : mcpStatus?.running ? 'Stop' : 'Start'}
            color={mcpStatus?.running ? 'red' : undefined}
            variant="outline"
            onClick={handleMcpToggle}
            disabled={mcpLoading}
          />
        </div>

        <p className="text-tan-muted text-xs">
          Exposes WAVS operations to AI assistants (Claude Desktop, Cursor, VS Code) via the Model Context Protocol.
        </p>

        {/* Auto-start toggle */}
        <label className="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={mcpAutoStart}
            onChange={(e) => setMcpAutoStart(e.target.checked)}
            className="w-4 h-4 accent-green-4"
          />
          <span className="text-beige-warm text-sm">Auto-start when WAVS node starts</span>
        </label>

        {/* Bearer token */}
        <div className="flex flex-col gap-1">
          <label className="text-tan-muted text-xs">Bearer token (for write operations)</label>
          <div className="flex gap-2">
            <input
              type="password"
              placeholder="Optional — leave blank for read-only access"
              value={mcpToken}
              onChange={(e) => setMcpToken(e.target.value)}
              className="flex-1 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
            />
            <Button
              text="Generate"
              variant="outline"
              onClick={() => {
                const bytes = new Uint8Array(24);
                crypto.getRandomValues(bytes);
                setMcpToken(btoa(String.fromCharCode(...bytes)).replace(/[+/=]/g, (c) => ({ '+': '-', '/': '_', '=': '' }[c] ?? c)));
              }}
            />
          </div>
        </div>

        <Button
          text="Save MCP Settings"
          variant="outline"
          onClick={handleMcpSaveSettings}
        />

        {/* Config snippet */}
        <div className="flex flex-col gap-1">
          <span className="text-tan-muted text-xs">Claude Desktop / Cursor config snippet:</span>
          <pre className="text-xs font-mono text-beige-warm bg-charcoal-darkest rounded p-3 overflow-x-auto whitespace-pre-wrap">{
`{
  "mcpServers": {
    "wavs": {
      "command": "${mcpBinaryPath ?? '/path/to/wavs-mcp'}",
      "args": ["--wavs-url", "${wavsUrl}"${mcpToken.trim() ? `,\n               "--token", "${mcpToken.trim()}"` : ''}]
    }
  }
}`
          }</pre>
          {!mcpBinaryPath && (
            <p className="text-tan-muted text-xs mt-1">
              Binary not found. Build it with: <span className="font-mono">cargo build --release -p wavs-mcp</span>
            </p>
          )}
        </div>

        {/* Register with Claude Code */}
        <div className="flex flex-col gap-2">
          <label className="text-tan-muted text-xs font-medium">Register with Claude Code</label>
          <p className="text-tan-muted text-xs">
            Add wavs-mcp to a Claude Code project so MCP tools are available there.
          </p>
          <div className="flex gap-2">
            <input
              type="text"
              value={claudeProjectPath}
              onChange={(e) => setClaudeProjectPath(e.target.value)}
              placeholder="/path/to/your-project"
              className="flex-1 px-3 py-2 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
            />
            <Button
              text="Browse..."
              variant="outline"
              onClick={async () => {
                const path = await pickFolder();
                if (path) setClaudeProjectPath(path);
              }}
            />
            <Button
              text={claudeRegisterLoading ? '...' : 'Register'}
              variant="outline"
              onClick={handleRegisterClaude}
              disabled={claudeRegisterLoading || !mcpStatus?.running || !claudeProjectPath.trim()}
            />
          </div>
          {claudeRegisterResult && (
            <p className="text-green-4 text-xs">
              Registered for {claudeRegisterResult}. Restart Claude Code to pick up the change.
            </p>
          )}
          {claudeRegisterError && (
            <p className="text-red-4 text-xs">{claudeRegisterError}</p>
          )}
        </div>

        {mcpError && <p className="text-red-4 text-sm">{mcpError}</p>}
      </div>

      {/* Reset App State */}
      <div className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
        <h2 className="text-beige-light text-lg font-semibold">Reset App State</h2>
        <p className="text-tan-muted text-sm">
          Remove all registered services and saved registries from the app. Useful when restarting a local chain (e.g. Anvil) where previous contract addresses no longer exist.
        </p>

        {!showClearServicesConfirm && (
          <Button
            text="Clear All Services & Registries"
            color="red"
            variant="outline"
            onClick={() => setShowClearServicesConfirm(true)}
          />
        )}

        {showClearServicesConfirm && (
          <div className="flex flex-col gap-3 p-3 rounded bg-charcoal-darkest border border-red-2">
            <p className="text-sm text-red-4">
              This will stop all running services and clear all saved registries. They can be re-added from the Services page.
            </p>
            <div className="flex gap-3">
              <Button
                text="Cancel"
                variant="outline"
                onClick={() => setShowClearServicesConfirm(false)}
              />
              <Button
                text="Yes, Clear Everything"
                color="red"
                onClick={handleClearServices}
              />
            </div>
          </div>
        )}
      </div>

      {/* Error display */}
      {displayError && (
        <p className="text-red-4 text-base">{displayError}</p>
      )}
    </div>
  );
}
