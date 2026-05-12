import { create } from 'zustand';
import type {
  Settings,
  LogItem,
  UnifiedActivity,
  Service,
  ServiceId,
  TriggerEvent,
  SubmissionEvent,
  SubmissionErrorEvent,
} from '../types';
import { correlationKey } from '../types';

const MAX_LOG_ITEMS = 5000;
const MAX_ACTIVITY_ITEMS = 2000;

let activityIdCounter = 0;
export function nextActivityId(): number {
  return ++activityIdCounter;
}

interface AppState {
  // State
  settings: Settings;
  logList: LogItem[];
  activityMap: Map<string, UnifiedActivity>;
  activityList: UnifiedActivity[];
  services: Map<ServiceId, Service>;

  // Computed
  isSettingsComplete: () => boolean;
  getServiceLabel: (serviceId: ServiceId) => string;

  // Actions
  setSettings: (settings: Settings) => void;
  addLogItem: (item: LogItem) => void;
  handleTrigger: (event: TriggerEvent) => void;
  handleSubmission: (event: SubmissionEvent) => void;
  handleSubmissionError: (event: SubmissionErrorEvent) => void;
  setServices: (serviceMap: Map<string, Service>) => void;
  removeService: (serviceId: ServiceId) => void;
  clearLogs: () => void;
  clearActivity: () => void;
}

function deriveActivityList(map: Map<string, UnifiedActivity>): UnifiedActivity[] {
  return Array.from(map.values());
}

function enforceMaxActivities(map: Map<string, UnifiedActivity>): Map<string, UnifiedActivity> {
  if (map.size <= MAX_ACTIVITY_ITEMS) return map;
  // Remove oldest entries by triggerTs
  const entries = Array.from(map.entries());
  entries.sort((a, b) => a[1].triggerTs - b[1].triggerTs);
  const trimmed = new Map(entries.slice(entries.length - MAX_ACTIVITY_ITEMS));
  return trimmed;
}

export const useAppStore = create<AppState>((set, get) => ({
  // Initial state
  settings: { wavs_home: null, saved_registries: [], saved_service_managers: [], saved_services: [], mcp_enabled: false, mcp_auto_start: false, mcp_token: null, env_vars: {} },
  logList: [],
  activityMap: new Map(),
  activityList: [],
  services: new Map(),

  // Computed
  isSettingsComplete: () => {
    return get().settings.wavs_home !== null;
  },

  getServiceLabel: (serviceId: ServiceId) => {
    const service = get().services.get(serviceId);
    return service?.name ?? 'unknown';
  },

  // Actions
  setSettings: (settings) => set({ settings }),

  addLogItem: (item) =>
    set((state) => {
      const next = [...state.logList, item];
      if (next.length > MAX_LOG_ITEMS) {
        return { logList: next.slice(next.length - MAX_LOG_ITEMS) };
      }
      return { logList: next };
    }),

  handleTrigger: (event: TriggerEvent) =>
    set((state) => {
      const action = event.action;
      const serviceId = action.config.service_id;
      const workflowId = action.config.workflow_id;
      const triggerData = action.data;
      const key = correlationKey(serviceId, workflowId, triggerData);

      // Check if this workflow has submission configured
      const service = state.services.get(serviceId);
      const workflow = service?.workflows[workflowId];
      const hasSubmission = workflow?.submit !== 'none';

      const newMap = new Map(state.activityMap);
      newMap.set(key, {
        id: nextActivityId(),
        correlationKey: key,
        triggerTs: Date.now(),
        submissionTs: null,
        status: hasSubmission ? 'pending' : 'executed',
        serviceId,
        workflowId,
        triggerData,
        triggerConfig: action.config,
        txHash: null,
        errorMessage: null,
      });

      const trimmed = enforceMaxActivities(newMap);
      return { activityMap: trimmed, activityList: deriveActivityList(trimmed) };
    }),

  handleSubmission: (event: SubmissionEvent) =>
    set((state) => {
      const key = correlationKey(event.service_id, event.workflow_id, event.trigger_data);
      const newMap = new Map(state.activityMap);
      const existing = newMap.get(key);

      if (existing) {
        newMap.set(key, {
          ...existing,
          status: 'confirmed',
          txHash: event.tx_hash ?? null,
          submissionTs: Date.now(),
        });
      } else {
        // Orphaned submission -- create standalone entry
        newMap.set(key, {
          id: nextActivityId(),
          correlationKey: key,
          triggerTs: Date.now(),
          submissionTs: Date.now(),
          status: 'confirmed',
          serviceId: event.service_id,
          workflowId: event.workflow_id,
          triggerData: event.trigger_data,
          txHash: event.tx_hash ?? null,
          errorMessage: null,
        });
      }

      return { activityMap: newMap, activityList: deriveActivityList(newMap) };
    }),

  handleSubmissionError: (event: SubmissionErrorEvent) =>
    set((state) => {
      const key = correlationKey(event.service_id, event.workflow_id, event.trigger_data);
      const newMap = new Map(state.activityMap);
      const existing = newMap.get(key);

      if (existing) {
        newMap.set(key, {
          ...existing,
          status: 'error',
          errorMessage: event.error_message,
          submissionTs: Date.now(),
        });
      } else {
        // Orphaned error -- create standalone entry
        newMap.set(key, {
          id: nextActivityId(),
          correlationKey: key,
          triggerTs: Date.now(),
          submissionTs: Date.now(),
          status: 'error',
          serviceId: event.service_id,
          workflowId: event.workflow_id,
          triggerData: event.trigger_data,
          txHash: null,
          errorMessage: event.error_message,
        });
      }

      return { activityMap: newMap, activityList: deriveActivityList(newMap) };
    }),

  setServices: (serviceMap) =>
    set({ services: serviceMap }),

  removeService: (serviceId) =>
    set((state) => {
      const newServices = new Map(state.services);
      newServices.delete(serviceId);
      return { services: newServices };
    }),

  clearLogs: () => set({ logList: [] }),

  clearActivity: () => set({ activityMap: new Map(), activityList: [] }),
}));
