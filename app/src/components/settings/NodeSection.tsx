import { useState, useEffect, useCallback } from 'react';
import { Button, TomlEditor } from '../atoms';
import { readWavsToml, writeWavsToml, setWavsHome, getSettings } from '../../tauri';
import { useAppStore } from '../../stores/appStore';

interface NodeSectionProps {
  wavsHome: string | null;
  onUnsavedChange: (hasChanges: boolean) => void;
  onChanged: () => void;
  onError: (msg: string | null) => void;
}

export function NodeSection({ wavsHome, onUnsavedChange, onChanged, onError }: NodeSectionProps) {
  const [tomlContent, setTomlContent] = useState('');
  const [savedContent, setSavedContent] = useState('');
  const [tomlLoading, setTomlLoading] = useState(false);
  const [tomlError, setTomlError] = useState<string | null>(null);
  const [tomlSaveSuccess, setTomlSaveSuccess] = useState(false);

  const loadToml = useCallback(async () => {
    if (!wavsHome) return;
    setTomlLoading(true);
    setTomlError(null);
    setTomlSaveSuccess(false);
    try {
      const content = await readWavsToml();
      setTomlContent(content);
      setSavedContent(content);
    } catch (err) {
      setTomlError(err instanceof Error ? err.message : typeof err === 'string' ? err : JSON.stringify(err));
    } finally {
      setTomlLoading(false);
    }
  }, [wavsHome]);

  useEffect(() => {
    loadToml();
  }, [loadToml]);

  // Notify parent when unsaved changes state changes
  useEffect(() => {
    onUnsavedChange(tomlContent !== savedContent);
  }, [tomlContent, savedContent, onUnsavedChange]);

  const handleSaveToml = async () => {
    setTomlError(null);
    setTomlSaveSuccess(false);
    try {
      await writeWavsToml(tomlContent);
      setSavedContent(tomlContent);
      setTomlSaveSuccess(true);
      onChanged();
    } catch (err) {
      setTomlError(err instanceof Error ? err.message : typeof err === 'string' ? err : JSON.stringify(err));
    }
  };

  const handleReloadToml = async () => {
    await loadToml();
  };

  const handleBrowse = async () => {
    onError(null);
    try {
      const path = await setWavsHome();
      if (path) {
        console.log('Changed wavs_home to', path);
        // Re-fetch settings so the UI updates immediately
        const updated = await getSettings();
        useAppStore.getState().setSettings(updated);
        onChanged();
      }
    } catch (err) {
      onError(err instanceof Error ? err.message : typeof err === 'string' ? err : JSON.stringify(err));
    }
  };

  const hasUnsavedChanges = tomlContent !== savedContent;

  return (
    <>
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
            value={wavsHome ?? ''}
            className="flex-1 px-4 py-3 rounded-md bg-charcoal-dark border border-charcoal-light text-beige-warm font-mono text-sm outline-none"
          />
          <Button text="Browse..." onClick={handleBrowse} />
        </div>
      </div>

      {/* TOML Editor */}
      {wavsHome && (
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
    </>
  );
}
