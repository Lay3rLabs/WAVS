import { useState, useEffect, useMemo, useRef } from 'react';
import { Button } from '../../components/atoms';
import { useAppStore } from '../../stores/appStore';
import { saveEnvVars } from '../../tauri';
import { errorMessage } from '../../utils/error';

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

export function EnvVariablesSection() {
  const settings = useAppStore((state) => state.settings);

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

  // Sync env vars from settings store on load
  useEffect(() => {
    setEnvVars(settings.env_vars ?? {});
  }, [settings.env_vars]);

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

  return (
    <div id="env-vars" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
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
  );
}
