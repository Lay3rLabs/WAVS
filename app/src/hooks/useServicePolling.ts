import { useEffect } from 'react';
import { useAppStore } from '../stores/appStore';
import { getServices } from '../tauri/commands';
import { buildServiceMap } from '../types';

const POLL_INTERVAL_MS = 5000;

/**
 * Poll for service updates every 5 seconds.
 * Use on pages that display service data.
 */
export function useServicePolling() {
  const setServices = useAppStore((s) => s.setServices);

  useEffect(() => {
    let active = true;

    const poll = async () => {
      try {
        const services = await getServices();
        if (active) setServices(await buildServiceMap(services));
      } catch {
        // WAVS may not be running
      }
    };

    // Initial fetch
    poll();
    const interval = setInterval(poll, POLL_INTERVAL_MS);

    return () => {
      active = false;
      clearInterval(interval);
    };
  }, [setServices]);
}
