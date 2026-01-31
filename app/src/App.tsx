import { useEffect, useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Header, Body } from './components/layout';
import { ModalContainer } from './components/atoms';
import { Settings, Logs, Services, Triggers, Submissions, NotFound } from './pages';
import { useAppStore } from './stores/appStore';
import { getSettings, startWavs } from './tauri';
import { startListeners } from './tauri/listeners';

function AppContent() {
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

        // If settings are complete, start WAVS
        if (settings.wavs_home) {
          try {
            await startWavs();
          } catch (err) {
            console.warn('Failed to start WAVS:', err);
            // Don't fail initialization if WAVS fails to start
          }
        }

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
