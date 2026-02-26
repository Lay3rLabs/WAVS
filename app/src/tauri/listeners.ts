import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useAppStore, nextActivityId } from '../stores/appStore';
import { buildServiceMap, type SettingsEvent, type LogEvent, type TriggerEvent, type SubmissionEvent, type ServiceEvent, type LogLevel } from '../types';
import { getServices } from './commands';

// Event names matching the Rust backend
const EVENTS = {
  SETTINGS: 'settings',
  LOG: 'log',
  TRIGGER: 'trigger',
  SUBMISSION: 'submission',
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

  // Trigger listener -> ActivityItem
  const unlistenTrigger = await listen<TriggerEvent>(EVENTS.TRIGGER, (event) => {
    const action = event.payload.action;
    store.addActivity({
      id: nextActivityId(),
      ts: Date.now(),
      kind: 'trigger',
      serviceId: action.config.service_id,
      workflowId: action.config.workflow_id,
      triggerData: action.data,
      triggerConfig: action.config,
    });
  });
  unlistenFns.push(unlistenTrigger);

  // Submission listener -> ActivityItem
  const unlistenSubmission = await listen<SubmissionEvent>(EVENTS.SUBMISSION, (event) => {
    const payload = event.payload;
    store.addActivity({
      id: nextActivityId(),
      ts: Date.now(),
      kind: 'submission',
      serviceId: payload.service_id,
      workflowId: payload.workflow_id,
      triggerData: payload.trigger_data,
    });
  });
  unlistenFns.push(unlistenSubmission);

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
