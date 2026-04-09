import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useAppStore, nextActivityId } from '../stores/appStore';
import { useAgentStore } from '../stores/agentStore';
import { buildServiceMap, type SettingsEvent, type LogEvent, type TriggerEvent, type SubmissionEvent, type ServiceEvent, type LogLevel, type SubmissionFailedEvent } from '../types';
import { getServices } from './commands';
import { Toast } from '../components/atoms/Toast';

// Event names matching the Rust backend
const EVENTS = {
  SETTINGS: 'settings',
  LOG: 'log',
  TRIGGER: 'trigger',
  SUBMISSION: 'submission',
  SUBMISSION_FAILED: 'submission_failed',
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
      correlationId: action.correlation_id,
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
      correlationId: payload.correlation_id,
      txHash: payload.tx_hash,
      resultPayload: payload.result_payload,
    });
  });
  unlistenFns.push(unlistenSubmission);

  // Submission failed listener -> ActivityItem
  const unlistenSubmissionFailed = await listen<SubmissionFailedEvent>(EVENTS.SUBMISSION_FAILED, (event) => {
    const payload = event.payload;
    store.addActivity({
      id: nextActivityId(),
      ts: Date.now(),
      kind: 'submission_failed',
      serviceId: payload.service_id,
      workflowId: payload.workflow_id,
      correlationId: payload.correlation_id,
      error: payload.error,
    });
  });
  unlistenFns.push(unlistenSubmissionFailed);

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

  // Agent RPC event listener
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const unlistenAgentEvent = await listen<{ event: any }>('agent:event', (event) => {
    useAgentStore.getState().handleAgentEvent(event.payload.event);
  });
  unlistenFns.push(unlistenAgentEvent);

  // Agent status listener
  const unlistenAgentStatus = await listen<{ status: string; error?: string }>('agent:status', (event) => {
    useAgentStore.getState().handleStatusEvent(event.payload.status, event.payload.error);
  });
  unlistenFns.push(unlistenAgentStatus);

  // Agent UI control listener
  const unlistenAgentUiControl = await listen<{ action: string; payload: Record<string, unknown> }>('agent:ui_control', (event) => {
    const { action, payload } = event.payload;
    if (action === 'navigate') {
      window.dispatchEvent(new CustomEvent('agent:navigate', { detail: (payload as { path: string }).path }));
    } else if (action === 'copy_to_clipboard') {
      const text = (payload as { text: string }).text;
      navigator.clipboard.writeText(text).then(() => {
        Toast.success('Copied to clipboard');
      }).catch(() => {
        Toast.error('Failed to copy to clipboard');
      });
    } else if (action === 'toast') {
      const p = payload as { message: string; level?: string };
      if (p.level === 'error') {
        Toast.error(p.message);
      } else if (p.level === 'success') {
        Toast.success(p.message);
      } else if (p.level === 'warning') {
        Toast.warning(p.message);
      } else {
        Toast.info(p.message);
      }
    }
  });
  unlistenFns.push(unlistenAgentUiControl);

  console.log('Tauri event listeners started');
}

export function stopListeners(): void {
  unlistenFns.forEach((unlisten) => unlisten());
  unlistenFns = [];
  console.log('Tauri event listeners stopped');
}
