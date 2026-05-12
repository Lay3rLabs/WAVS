import { useState } from 'react';
import { Button } from '../../components/atoms';
import { useAppStore } from '../../stores/appStore';
import { setWavsHome } from '../../tauri';

interface WavsHomeSectionProps {
  onChanged: () => void;
}

export function WavsHomeSection({ onChanged }: WavsHomeSectionProps) {
  const settings = useAppStore((state) => state.settings);
  const [error, setError] = useState<string | null>(null);

  const handleBrowse = async () => {
    setError(null);
    try {
      const path = await setWavsHome();
      if (path) {
        console.log('Changed wavs_home to', path);
        onChanged();
      }
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div id="wavs-home" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
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
      {error && (
        <p className="text-red-4 text-sm">{error}</p>
      )}
    </div>
  );
}
