import { useEffect, useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Header, Body } from './components/layout';
import { ModalContainer } from './components/atoms';
import {
  Settings,
  Logs,
  Services,
  Triggers,
  Submissions,
  NotFound,
  WalletSetup,
  Health,
} from './pages';
import { useAppStore } from './stores/appStore';
import { useWalletStore } from './stores/walletStore';
import { getSettings, startWavs } from './tauri';
import { startListeners } from './tauri/listeners';

function MainAppContent() {
  const isSettingsComplete = useAppStore((state) => state.isSettingsComplete());

  return (
    <div className="h-full flex flex-col">
      <Header />
      <Routes>
        <Route element={<Body />}>
          <Route path="/settings" element={<Settings />} />
          <Route path="/logs" element={<Logs />} />
          <Route path="/services" element={<Services />} />
          <Route path="/triggers" element={<Triggers />} />
          <Route path="/submissions" element={<Submissions />} />
          <Route path="/poa-registry" element={<Navigate to="/services" replace />} />
          <Route path="/health" element={<Health />} />
          <Route path="/404" element={<NotFound />} />
          {/* Default route: go to settings if not complete, otherwise logs */}
          <Route
            path="/"
            element={
              <Navigate to={isSettingsComplete ? '/logs' : '/settings'} replace />
            }
          />
          <Route path="*" element={<Navigate to="/404" replace />} />
        </Route>
      </Routes>
      <ModalContainer />
    </div>
  );
}

function AppContent() {
  const settings = useAppStore((state) => state.settings);
  const { hasMnemonic, checkMnemonic } = useWalletStore();
  const [wavsStarted, setWavsStarted] = useState(false);
  const [walletChecked, setWalletChecked] = useState(false);

  // Check for mnemonic in keychain on mount
  useEffect(() => {
    const check = async () => {
      await checkMnemonic();
      setWalletChecked(true);
    };
    check();
  }, [checkMnemonic]);

  // Start WAVS after wallet is set up and wavs_home is set
  useEffect(() => {
    const startWavsIfReady = async () => {
      if (hasMnemonic && settings.wavs_home && !wavsStarted) {
        try {
          await startWavs();
          setWavsStarted(true);
        } catch (err) {
          console.warn('Failed to start WAVS:', err);
          // Still allow the app to function
          setWavsStarted(true);
        }
      }
    };
    startWavsIfReady();
  }, [hasMnemonic, settings.wavs_home, wavsStarted]);

  // Wait for wallet check to complete
  if (!walletChecked) {
    return (
      <div className="h-full flex items-center justify-center bg-charcoal-darkest">
        <div className="text-lg text-beige-warm">Loading...</div>
      </div>
    );
  }

  // If no mnemonic in keychain, show wallet setup
  if (!hasMnemonic) {
    return <WalletSetup />;
  }

  // Wallet is set up, show main app
  return <MainAppContent />;
}

function App() {
  const [initialized, setInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const setSettings = useAppStore((state) => state.setSettings);

  useEffect(() => {
    const init = async () => {
      try {
        // Load initial settings
        const settings = await getSettings();
        setSettings(settings);

        // Start event listeners
        await startListeners();

        setInitialized(true);
      } catch (err) {
        console.error('Failed to initialize app:', err);
        setError(String(err));
      }
    };

    init();
  }, [setSettings]);

  if (error) {
    return (
      <div className="h-full flex items-center justify-center bg-charcoal-darkest">
        <div className="p-8 rounded-lg bg-charcoal-dark border border-charcoal-light">
          <h1 className="text-xl font-bold text-red-4 mb-4">Initialization Error</h1>
          <p className="text-beige-warm">{error}</p>
        </div>
      </div>
    );
  }

  if (!initialized) {
    return (
      <div className="h-full flex items-center justify-center bg-charcoal-darkest">
        <div className="text-lg text-beige-warm">Loading...</div>
      </div>
    );
  }

  return (
    <BrowserRouter>
      <AppContent />
    </BrowserRouter>
  );
}

export default App;
