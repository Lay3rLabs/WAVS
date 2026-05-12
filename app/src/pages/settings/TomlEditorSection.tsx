import { useState, useEffect, useCallback } from 'react';
import { Button, TomlEditor } from '../../components/atoms';
import { useAppStore } from '../../stores/appStore';
import { readWavsToml, writeWavsToml } from '../../tauri';

interface TomlEditorSectionProps {
  onChanged: () => void;
}

export function TomlEditorSection({ onChanged }: TomlEditorSectionProps) {
  const settings = useAppStore((state) => state.settings);

  const [tomlContent, setTomlContent] = useState('');
  const [savedContent, setSavedContent] = useState('');
  const [tomlLoading, setTomlLoading] = useState(false);
  const [tomlError, setTomlError] = useState<string | null>(null);
  const [tomlSaveSuccess, setTomlSaveSuccess] = useState(false);
  const hasUnsavedChanges = tomlContent !== savedContent;

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
      onChanged();
    } catch (err) {
      setTomlError(String(err));
    }
  };

  const handleReloadToml = async () => {
    await loadToml();
  };

  return (
    <div id="toml-editor" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
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
  );
}
