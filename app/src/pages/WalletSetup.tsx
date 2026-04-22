import { useState } from 'react';
import { Button, TextArea } from '../components/atoms';
import { useWalletStore } from '../stores/walletStore';

type SetupMode = 'choose' | 'generate' | 'import' | 'confirm';

export function WalletSetup() {
  const [mode, setMode] = useState<SetupMode>('choose');
  const [importInput, setImportInput] = useState('');
  const [backupConfirmed, setBackupConfirmed] = useState(false);

  const {
    pendingMnemonic,
    derivedAddresses,
    isLoading,
    error,
    generateMnemonic,
    importMnemonic,
    saveMnemonic,
    clearPendingMnemonic,
    clearError,
  } = useWalletStore();

  const handleGenerateNew = async () => {
    clearError();
    setMode('generate');
    await generateMnemonic();
  };

  const handleImportExisting = () => {
    clearError();
    setMode('import');
  };

  const handleImportSubmit = async () => {
    clearError();
    const success = await importMnemonic(importInput);
    if (success) {
      setMode('confirm');
    }
  };

  const handleConfirmBackup = () => {
    setMode('confirm');
  };

  const handleBack = () => {
    clearError();
    clearPendingMnemonic();
    setImportInput('');
    setBackupConfirmed(false);
    setMode('choose');
  };

  const handleFinish = async () => {
    clearError();
    await saveMnemonic();
  };

  // Choose mode
  if (mode === 'choose') {
    return (
      <div className="h-full flex items-center justify-center bg-charcoal-darkest">
        <div className="flex flex-col items-center gap-8 p-12 rounded-xl bg-charcoal-dark border border-charcoal-light max-w-md">
          <div className="flex flex-col items-center gap-3">
            <h1 className="text-2xl font-bold text-cream-light">Wallet Setup</h1>
            <p className="text-beige-warm text-center">
              Create a new wallet or import an existing one.
              Your recovery phrase will be securely stored in your operating system's keychain.
            </p>
          </div>

          <div className="flex flex-col gap-4 w-full">
            <Button
              text="Generate New Wallet"
              color="purple"
              size="xlg"
              onClick={handleGenerateNew}
              disabled={isLoading}
            />
            <Button
              text="Import Existing Wallet"
              variant="outline"
              size="xlg"
              onClick={handleImportExisting}
              disabled={isLoading}
            />
          </div>

          {error && <p className="text-red-4 text-sm text-center">{error}</p>}
        </div>
      </div>
    );
  }

  // Generate mode - show mnemonic
  if (mode === 'generate') {
    return (
      <div className="h-full flex items-center justify-center bg-charcoal-darkest">
        <div className="flex flex-col items-center gap-6 p-12 rounded-xl bg-charcoal-dark border border-charcoal-light max-w-xl">
          <div className="flex flex-col items-center gap-3">
            <h1 className="text-2xl font-bold text-cream-light">
              Your Recovery Phrase
            </h1>
            <p className="text-beige-warm text-center">
              Write down these 24 words in order. Keep them safe and never share them.
            </p>
          </div>

          {isLoading ? (
            <div className="text-beige-warm">Generating...</div>
          ) : pendingMnemonic ? (
            <>
              <div className="grid grid-cols-4 gap-2 p-4 rounded-lg bg-charcoal-darkest border border-charcoal-light">
                {pendingMnemonic.split(' ').map((word, i) => (
                  <div
                    key={i}
                    className="flex items-center gap-2 p-2 rounded bg-charcoal-medium"
                  >
                    <span className="text-tan-muted text-xs w-5">{i + 1}.</span>
                    <span className="text-beige-warm font-mono text-sm">
                      {word}
                    </span>
                  </div>
                ))}
              </div>

              {derivedAddresses.length > 0 && (
                <div className="flex flex-col gap-2 w-full">
                  <span className="text-sm text-tan-muted">Preview addresses:</span>
                  {derivedAddresses.map((addr, i) => (
                    <div
                      key={i}
                      className="text-xs font-mono text-beige-warm bg-charcoal-darkest p-2 rounded"
                    >
                      {i}: {addr}
                    </div>
                  ))}
                </div>
              )}

              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={backupConfirmed}
                  onChange={(e) => setBackupConfirmed(e.target.checked)}
                  className="w-5 h-5 rounded border-charcoal-light bg-charcoal-darkest"
                />
                <span className="text-beige-warm text-sm">
                  I have written down my recovery phrase
                </span>
              </label>
            </>
          ) : null}

          {error && <p className="text-red-4 text-sm">{error}</p>}

          <div className="flex gap-4">
            <Button text="Back" variant="outline" onClick={handleBack} />
            <Button
              text="Continue"
              color="purple"
              onClick={handleConfirmBackup}
              disabled={!backupConfirmed || !pendingMnemonic}
            />
          </div>
        </div>
      </div>
    );
  }

  // Import mode
  if (mode === 'import') {
    return (
      <div className="h-full flex items-center justify-center bg-charcoal-darkest">
        <div className="flex flex-col items-center gap-6 p-12 rounded-xl bg-charcoal-dark border border-charcoal-light max-w-xl">
          <div className="flex flex-col items-center gap-3">
            <h1 className="text-2xl font-bold text-cream-light">
              Import Recovery Phrase
            </h1>
            <p className="text-beige-warm text-center">
              Enter your 12 or 24 word recovery phrase
            </p>
          </div>

          <TextArea
            placeholder="Enter your recovery phrase..."
            value={importInput}
            onChange={setImportInput}
            rows={4}
            className="w-full font-mono"
          />

          {error && <p className="text-red-4 text-sm">{error}</p>}

          <div className="flex gap-4">
            <Button text="Back" variant="outline" onClick={handleBack} />
            <Button
              text={isLoading ? 'Validating...' : 'Import'}
              color="purple"
              onClick={handleImportSubmit}
              disabled={isLoading || !importInput.trim()}
            />
          </div>
        </div>
      </div>
    );
  }

  // Confirm mode - final step before saving to keychain
  if (mode === 'confirm') {
    return (
      <div className="h-full flex items-center justify-center bg-charcoal-darkest">
        <div className="flex flex-col items-center gap-6 p-12 rounded-xl bg-charcoal-dark border border-charcoal-light max-w-xl">
          <div className="flex flex-col items-center gap-3">
            <h1 className="text-2xl font-bold text-cream-light">
              Confirm & Save
            </h1>
            <p className="text-beige-warm text-center">
              Your wallet will be stored securely in your operating system's keychain.
              This is protected by your device's authentication (Touch ID, login password, etc.).
            </p>
          </div>

          {derivedAddresses.length > 0 && (
            <div className="flex flex-col gap-2 w-full">
              <span className="text-sm text-tan-muted">Your addresses:</span>
              {derivedAddresses.map((addr, i) => (
                <div
                  key={i}
                  className="text-xs font-mono text-beige-warm bg-charcoal-darkest p-2 rounded"
                >
                  {i}: {addr}
                </div>
              ))}
            </div>
          )}

          {error && <p className="text-red-4 text-sm text-center">{error}</p>}

          <div className="flex gap-4">
            <Button text="Back" variant="outline" onClick={handleBack} />
            <Button
              text={isLoading ? 'Saving...' : 'Save to Keychain'}
              color="purple"
              onClick={handleFinish}
              disabled={isLoading}
            />
          </div>
        </div>
      </div>
    );
  }

  return null;
}
