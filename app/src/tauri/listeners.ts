import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useAppStore } from '../stores/appStore';
import type {
  SettingsEvent,
  LogEvent,
  TriggerEvent,
  SubmissionEvent,
  LogLevel,
} from '../types';

// Event names matching the Rust backend
const EVENTS = {
  SETTINGS: 'settings',
  LOG: 'log',
  TRIGGER: 'trigger',
  SUBMISSION: 'submission',
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

    // Parse level string to LogLevel
    const logLevel = (level.toUpperCase() as LogLevel) || 'INFO';

    store.addLogItem({
      ts: Date.now(),
      level: logLevel,
      target,
      fields,
    });
  });
  unlistenFns.push(unlistenLog);

  // Trigger listener
  const unlistenTrigger = await listen<TriggerEvent>(EVENTS.TRIGGER, (event) => {
    store.addTrigger(event.payload.action);
  });
  unlistenFns.push(unlistenTrigger);

  // Submission listener
  const unlistenSubmission = await listen<SubmissionEvent>(EVENTS.SUBMISSION, (event) => {
    store.addSubmission(event.payload);
  });
  unlistenFns.push(unlistenSubmission);

  console.log('Tauri event listeners started');
}

export function stopListeners(): void {
  unlistenFns.forEach((unlisten) => unlisten());
  unlistenFns = [];
  console.log('Tauri event listeners stopped');
}
