import { useEffect, useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Header, Body } from './components/layout';
import { ModalContainer, ToastContainer } from './components/atoms';
import {
  Settings,
  Logs,
  Activity,
  NotFound,
  WalletSetup,
  Health,
  ComponentsPage,
} from './pages';
import {
  ServicesLayout,
  ServiceListPage,
  ServiceBuilderPage,
  ServiceDetailPage,
  ServiceEditorPage,
} from './pages/services';
import { useAppStore } from './stores/appStore';
import { useWalletStore } from './stores/walletStore';
import { getSettings, startWavs, getServices } from './tauri';
import { startListeners, stopListeners } from './tauri/listeners';
import { buildServiceMap } from './types';

function MainAppContent() {
  const isSettingsComplete = useAppStore((state) => state.isSettingsComplete());

  return (
    <div className="h-full flex flex-col">
      <Header />
      <Routes>
        <Route element={<Body />}>
          <Route path="/settings" element={<Settings />} />
          <Route path="/logs" element={<Logs />} />
          <Route path="/services" element={<ServicesLayout />}>
            <Route index element={<ServiceListPage />} />
            <Route path="new" element={<ServiceBuilderPage />} />
            <Route path=":chainId/:address" element={<ServiceDetailPage />} />
            <Route path=":chainId/:address/edit" element={<ServiceEditorPage />} />
          </Route>
          <Route path="/components" element={<ComponentsPage />} />
          <Route path="/activity" element={<Activity />} />
          <Route path="/triggers" element={<Navigate to="/activity" replace />} />
          <Route path="/submissions" element={<Navigate to="/activity" replace />} />
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
      <ToastContainer />
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

  const setServices = useAppStore((state) => state.setServices);

  // Start WAVS after wallet is set up and wavs_home is set
  useEffect(() => {
    const startWavsIfReady = async () => {
      if (hasMnemonic && settings.wavs_home && !wavsStarted) {
        try {
          await startWavs();
          setWavsStarted(true);
          // Refresh services now that WAVS is running
          try {
            const services = await getServices();
            setServices(await buildServiceMap(services));
          } catch {
            // Services may not be available yet
          }
        } catch (err) {
          console.warn('Failed to start WAVS:', err);
          // Still allow the app to function
          setWavsStarted(true);
        }
      }
    };
    startWavsIfReady();
  }, [hasMnemonic, settings.wavs_home, wavsStarted, setServices]);

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
  const setServices = useAppStore((state) => state.setServices);

  useEffect(() => {
    let cancelled = false;

    const init = async () => {
      try {
        // Load initial settings
        const settings = await getSettings();
        if (cancelled) return;
        setSettings(settings);

        // Start event listeners
        await startListeners();
        if (cancelled) {
          stopListeners();
          return;
        }

        // Load services early so getServiceLabel() works everywhere
        try {
          const services = await getServices();
          if (!cancelled) setServices(await buildServiceMap(services));
        } catch {
          // WAVS may not be running yet -- services will load when it starts
        }

        if (!cancelled) setInitialized(true);
      } catch (err) {
        if (!cancelled) {
          console.error('Failed to initialize app:', err);
          setError(String(err));
        }
      }
    };

    init();
    return () => {
      cancelled = true;
      stopListeners();
    };
  }, [setSettings, setServices]);

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
