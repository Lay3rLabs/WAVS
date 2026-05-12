import { useState } from 'react';
import { Button } from '../../components/atoms';
import { usePOAStore } from '../../stores/poaStore';
import { clearPersistedServices } from '../../tauri';

export function ResetAppSection() {
  const [showClearServicesConfirm, setShowClearServicesConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <div id="reset" className="flex flex-col gap-4 p-4 rounded-lg bg-charcoal-medium border border-charcoal-light">
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
              text="Keep Everything"
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

      {error && (
        <p className="text-red-4 text-sm">{error}</p>
      )}
    </div>
  );
}
