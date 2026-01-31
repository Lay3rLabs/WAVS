import { useState } from 'react';
import { Button } from '../components/atoms';
import { useAppStore } from '../stores/appStore';
import { useWalletStore } from '../stores/walletStore';
import { setWavsHome, restart } from '../tauri';

export function Settings() {
  const settings = useAppStore((state) => state.settings);
  const {
    hasMnemonic,
    isLoading,
    error: walletError,
    getMnemonic,
    deleteMnemonic,
    clearError,
  } = useWalletStore();

  const [changed, setChanged] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showMnemonic, setShowMnemonic] = useState(false);
  const [exportedMnemonic, setExportedMnemonic] = useState<string | null>(null);
  const [showResetConfirm, setShowResetConfirm] = useState(false);

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
    } catch (err) {
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
      // The app will automatically show the wallet setup screen
    } catch (err) {
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

        <div className="flex flex-col gap-2">
          <label className="text-tan-muted text-sm">Status</label>
          <div className="text-sm text-beige-warm bg-charcoal-dark p-2 rounded">
            {hasMnemonic ? (
              <span className="text-green-4">Wallet configured (stored in OS keychain)</span>
            ) : (
              <span className="text-red-4">No wallet configured</span>
            )}
          </div>
        </div>

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

      {/* Error display */}
      {displayError && (
        <p className="text-red-4 text-base">{displayError}</p>
      )}
    </div>
  );
}
