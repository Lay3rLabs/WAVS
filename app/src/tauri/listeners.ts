import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useAppStore } from '../stores/appStore';
import { buildServiceMap, type SettingsEvent, type LogEvent, type TriggerEvent, type SubmissionEvent, type SubmissionErrorEvent, type ServiceEvent, type LogLevel } from '../types';
import { getServices } from './commands';

// Event names matching the Rust backend
const EVENTS = {
  SETTINGS: 'settings',
  LOG: 'log',
  TRIGGER: 'trigger',
  SUBMISSION: 'submission',
  SUBMISSION_ERROR: 'submission_error',
  SERVICE: 'service',
} as const;

let unlistenFns: UnlistenFn[] = [];

export async function startListeners(): Promise<void> {
  const store = useAppStore.getState();

  // Settings listener
  const unlistenSettings = await listen<SettingsEvent>(EVENTS.SETTINGS, (event) => {
    store.setSettings(event.payload.settings);
  });
  unlistenFns.push(unlistenSettings);

  // Log listener
  const unlistenLog = await listen<LogEvent>(EVENTS.LOG, (event) => {
    const { level, target, fields } = event.payload;
    const logLevel = (level.toUpperCase() as LogLevel) || 'INFO';

    store.addLogItem({
      ts: Date.now(),
      level: logLevel,
      target,
      fields,
    });
  });
  unlistenFns.push(unlistenLog);

  // Trigger listener -> UnifiedActivity (pending)
  const unlistenTrigger = await listen<TriggerEvent>(EVENTS.TRIGGER, (event) => {
    store.handleTrigger(event.payload);
  });
  unlistenFns.push(unlistenTrigger);

  // Submission listener -> UnifiedActivity (confirmed)
  const unlistenSubmission = await listen<SubmissionEvent>(EVENTS.SUBMISSION, (event) => {
    store.handleSubmission(event.payload);
  });
  unlistenFns.push(unlistenSubmission);

  // Submission error listener -> UnifiedActivity (error)
  const unlistenSubmissionError = await listen<SubmissionErrorEvent>(EVENTS.SUBMISSION_ERROR, (event) => {
    store.handleSubmissionError(event.payload);
  });
  unlistenFns.push(unlistenSubmissionError);

  // Service listener -> re-fetch service list
  const unlistenService = await listen<ServiceEvent>(EVENTS.SERVICE, async (_event) => {
    try {
      const services = await getServices();
      store.setServices(await buildServiceMap(services));
    } catch (err) {
      console.warn('Failed to refresh services after service event:', err);
    }
  });
  unlistenFns.push(unlistenService);

  console.log('Tauri event listeners started');
}

export function stopListeners(): void {
  unlistenFns.forEach((unlisten) => unlisten());
  unlistenFns = [];
  console.log('Tauri event listeners stopped');
}
