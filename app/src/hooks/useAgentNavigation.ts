import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

/**
 * Hook that listens for `agent:navigate` CustomEvents dispatched by the
 * agent UI control listener (in listeners.ts) and forwards them to
 * React Router's navigate function.
 *
 * Mount this once in a component that's inside the Router context.
 */
export function useAgentNavigation() {
  const navigate = useNavigate();

  useEffect(() => {
    const handler = (e: Event) => {
      const path = (e as CustomEvent<string>).detail;
      if (path) {
        navigate(path);
      }
    };

    window.addEventListener('agent:navigate', handler);
    return () => window.removeEventListener('agent:navigate', handler);
  }, [navigate]);
}
