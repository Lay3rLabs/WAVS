import { useState } from 'react';
import { TextInput, Button, Dropdown, Toast, type DropdownOption } from '../atoms';
import type { ComponentDraft } from '../../stores/serviceBuilderStore';
import { getComponentDigest, publishComponent } from '../../tauri/commands';
import { getErrorMessage } from '../../types';
import { open } from '@tauri-apps/plugin-dialog';

type SourceType = 'registry' | 'download' | 'digest';

const SOURCE_OPTIONS: DropdownOption<SourceType>[] = [
  { label: 'Registry', value: 'registry' },
  { label: 'Download', value: 'download' },
  { label: 'Digest', value: 'digest' },
];

type HttpHostMode = 'all' | 'none' | 'specific';

const HTTP_HOST_OPTIONS: DropdownOption<HttpHostMode>[] = [
  { label: 'None', value: 'none' },
  { label: 'All', value: 'all' },
  { label: 'Specific', value: 'specific' },
];

interface ComponentEditorProps {
  component: ComponentDraft;
  onChange: (component: ComponentDraft) => void;
}

export function ComponentEditor({ component, onChange }: ComponentEditorProps) {
  const [lookingUpDigest, setLookingUpDigest] = useState(false);
  const [newConfigKey, setNewConfigKey] = useState('');
  const [newConfigValue, setNewConfigValue] = useState('');
  const [newEnvKey, setNewEnvKey] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);

  const update = (updates: Partial<ComponentDraft>) => {
    onChange({ ...component, ...updates });
  };

  const hasAdvanced =
    component.fuelLimit !== '' ||
    component.timeLimitSeconds !== '';

  const handleLookupDigest = async () => {
    if (!component.package) {
      Toast.error('Please enter a package name.');
      return;
    }
    setLookingUpDigest(true);
    try {
      const result = await getComponentDigest(
        component.domain || null,
        component.package,
        component.version || null
      );
      update({ digest: result.digest, version: result.resolved_version });
      Toast.info(`Digest resolved: ${result.digest.slice(0, 16)}... (v${result.resolved_version})`);
    } catch (err) {
      Toast.error(`Failed to lookup digest: ${getErrorMessage(err)}`);
    } finally {
      setLookingUpDigest(false);
    }
  };

  const handleUploadWasm = async () => {
    try {
      const filePath = await open({
        multiple: false,
        filters: [{ name: 'WebAssembly', extensions: ['wasm'] }],
      });
      if (!filePath) return;

      const digest = await publishComponent(filePath);
      update({ digest, sourceType: 'digest' });
      Toast.info(`Component digest: ${digest.slice(0, 16)}...`);
    } catch (err) {
      Toast.error(`Failed to process wasm file: ${getErrorMessage(err)}`);
    }
  };

  const addConfigPair = () => {
    if (!newConfigKey) return;
    update({ config: { ...component.config, [newConfigKey]: newConfigValue } });
    setNewConfigKey('');
    setNewConfigValue('');
  };

  const removeConfigKey = (key: string) => {
    const newConfig = { ...component.config };
    delete newConfig[key];
    update({ config: newConfig });
  };

  const addEnvKey = () => {
    if (!newEnvKey) return;
    const key = newEnvKey.startsWith('WAVS_ENV_') ? newEnvKey : `WAVS_ENV_${newEnvKey}`;
    if (!component.envKeys.includes(key)) {
      update({ envKeys: [...component.envKeys, key] });
    }
    setNewEnvKey('');
  };

  const removeEnvKey = (key: string) => {
    update({ envKeys: component.envKeys.filter((k) => k !== key) });
  };

  const addSpecificHost = () => {
    update({ specificHosts: [...component.specificHosts, ''] });
  };

  const updateSpecificHost = (index: number, value: string) => {
    const newHosts = [...component.specificHosts];
    newHosts[index] = value;
    update({ specificHosts: newHosts });
  };

  const removeSpecificHost = (index: number) => {
    update({ specificHosts: component.specificHosts.filter((_, i) => i !== index) });
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Source Type */}
      <div className="flex flex-col gap-2">
        <label className="text-beige-warm text-sm">Source Type</label>
        <Dropdown
          options={SOURCE_OPTIONS}
          value={component.sourceType}
          onChange={(v) => update({ sourceType: v })}
          size="sm"
        />
      </div>

      {/* Registry source */}
      {component.sourceType === 'registry' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Domain (optional)</label>
            <TextInput placeholder="e.g. wa.dev" value={component.domain} onChange={(v) => update({ domain: v })} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Package</label>
            <TextInput placeholder="e.g. example:my-component" value={component.package} onChange={(v) => update({ package: v })} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Version (optional, defaults to latest)</label>
            <TextInput placeholder="e.g. 0.1.0" value={component.version} onChange={(v) => update({ version: v })} />
          </div>
          <div className="flex items-center gap-3">
            <Button text={lookingUpDigest ? 'Looking up...' : 'Lookup Digest'} color="purple" size="sm" onClick={handleLookupDigest} disabled={lookingUpDigest} />
            <Button text="Upload .wasm" color="primary" size="sm" onClick={handleUploadWasm} />
          </div>
          {component.digest && (
            <div className="flex flex-col gap-1">
              <label className="text-beige-warm text-sm">Digest</label>
              <span className="text-xs text-tan-muted font-mono break-all">{component.digest}</span>
            </div>
          )}
        </div>
      )}

      {/* Download source */}
      {component.sourceType === 'download' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">URI</label>
            <TextInput placeholder="https://..." value={component.uri} onChange={(v) => update({ uri: v })} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Digest</label>
            <TextInput placeholder="sha256 hex digest" value={component.digest} onChange={(v) => update({ digest: v })} />
          </div>
          <Button text="Upload .wasm to get digest" color="primary" size="sm" onClick={handleUploadWasm} />
        </div>
      )}

      {/* Digest source */}
      {component.sourceType === 'digest' && (
        <div className="flex flex-col gap-3 p-4 rounded bg-charcoal-dark border border-charcoal-light">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Digest</label>
            <TextInput placeholder="sha256 hex digest" value={component.digest} onChange={(v) => update({ digest: v })} />
          </div>
          <Button text="Upload .wasm to get digest" color="primary" size="sm" onClick={handleUploadWasm} />
        </div>
      )}

      {/* Permissions */}
      <div className="flex flex-col gap-3">
        <h5 className="text-beige-warm text-sm font-medium">Permissions</h5>
        <div className="flex flex-col gap-2">
          <label className="text-beige-warm text-sm">Allowed HTTP Hosts</label>
          <Dropdown
            options={HTTP_HOST_OPTIONS}
            value={component.httpHosts}
            onChange={(v) => update({ httpHosts: v })}
            size="sm"
          />
        </div>
        {component.httpHosts === 'specific' && (
          <div className="flex flex-col gap-2 pl-4">
            {component.specificHosts.map((host, i) => (
              <div key={i} className="flex items-center gap-2">
                <TextInput placeholder="hostname" value={host} onChange={(v) => updateSpecificHost(i, v)} className="flex-1" />
                <button type="button" onClick={() => removeSpecificHost(i)} className="text-red-3 hover:text-red-4 text-sm cursor-pointer">Remove</button>
              </div>
            ))}
            <button type="button" onClick={addSpecificHost} className="self-start text-sm text-purple-2 hover:text-purple-3 cursor-pointer">+ Add Host</button>
          </div>
        )}
        <label className="flex items-center gap-2 text-beige-warm text-sm cursor-pointer">
          <input
            type="checkbox"
            checked={component.fileSystem}
            onChange={(e) => update({ fileSystem: e.target.checked })}
            className="accent-purple-1"
          />
          File System Access
        </label>
      </div>

      {/* Config */}
      <div className="flex flex-col gap-2">
        <h5 className="text-beige-warm text-sm font-medium">Config (key-value pairs)</h5>
        {Object.entries(component.config).map(([key, value]) => (
          <div key={key} className="flex items-center gap-2 text-sm">
            <span className="text-beige-warm font-mono">{key}</span>
            <span className="text-tan-muted">=</span>
            <span className="text-beige-warm font-mono">{value}</span>
            <button type="button" onClick={() => removeConfigKey(key)} className="text-red-3 hover:text-red-4 cursor-pointer">Remove</button>
          </div>
        ))}
        <div className="flex items-center gap-2">
          <TextInput placeholder="Key" value={newConfigKey} onChange={setNewConfigKey} className="flex-1" />
          <TextInput placeholder="Value" value={newConfigValue} onChange={setNewConfigValue} className="flex-1" />
          <button type="button" onClick={addConfigPair} className="text-sm text-purple-2 hover:text-purple-3 cursor-pointer">Add</button>
        </div>
      </div>

      {/* Env Keys */}
      <div className="flex flex-col gap-2">
        <h5 className="text-beige-warm text-sm font-medium">Environment Keys</h5>
        {component.envKeys.map((key) => (
          <div key={key} className="flex items-center gap-2 text-sm">
            <span className="text-beige-warm font-mono">{key}</span>
            <button type="button" onClick={() => removeEnvKey(key)} className="text-red-3 hover:text-red-4 cursor-pointer">Remove</button>
          </div>
        ))}
        <div className="flex items-center gap-2">
          <TextInput placeholder="e.g. MY_API_KEY (WAVS_ENV_ prefix auto-added)" value={newEnvKey} onChange={setNewEnvKey} className="flex-1" />
          <button type="button" onClick={addEnvKey} className="text-sm text-purple-2 hover:text-purple-3 cursor-pointer">Add</button>
        </div>
      </div>

      {/* Advanced toggle — Fuel Limit & Time Limit only */}
      <button
        type="button"
        onClick={() => setShowAdvanced(!showAdvanced)}
        className="self-start text-sm text-tan-muted hover:text-beige-warm transition-colors cursor-pointer"
      >
        {showAdvanced ? '− Advanced' : '+ Advanced'}
        {!showAdvanced && hasAdvanced && (
          <span className="ml-1 text-purple-3">(configured)</span>
        )}
      </button>

      {showAdvanced && (
        <div className="grid grid-cols-2 gap-3">
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Fuel Limit (optional)</label>
            <TextInput kind="number" placeholder="e.g. 1000000" value={component.fuelLimit} onChange={(v) => update({ fuelLimit: v })} />
          </div>
          <div className="flex flex-col gap-2">
            <label className="text-beige-warm text-sm">Time Limit Seconds (optional)</label>
            <TextInput kind="number" placeholder="e.g. 30" value={component.timeLimitSeconds} onChange={(v) => update({ timeLimitSeconds: v })} />
          </div>
        </div>
      )}
    </div>
  );
}
