import { useState } from 'react';
import { Button } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { setWavsHome, restart } from '../tauri';

export function Settings() {
  const settings = useAppStore((state) => state.settings);
  const [changed, setChanged] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <div className="flex flex-col gap-3 max-h-[calc(100vh-12rem)] overflow-y-auto pr-2">
      {/* Restart warning */}
      {changed && (
        <div className="flex gap-4 mb-8 items-center">
          <div className="flex-1 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
            <p className="text-lg text-beige-light">
              ⚠️ Restart for changes to take effect.
            </p>
          </div>
          <Button
            text="Restart Application"
            color="red"
            onClick={handleRestart}
          />
        </div>
      )}

      {/* WAVS Home Directory */}
      <div className="flex flex-col gap-4">
        <label className="text-beige-light text-lg font-semibold">
          WAVS Home Directory
        </label>
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

        {/* Error display */}
        {error && (
          <p className="text-red-4 text-base">{error}</p>
        )}
      </div>
    </div>
  );
}
