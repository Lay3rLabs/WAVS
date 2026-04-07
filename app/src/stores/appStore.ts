import { create } from 'zustand';
import type {
  Settings,
  LogItem,
  ActivityItem,
  Service,
  ServiceId,
} from '../types';

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
  activityList: ActivityItem[];
  services: Map<ServiceId, Service>;

  // Computed
  isSettingsComplete: () => boolean;
  getServiceLabel: (serviceId: ServiceId) => string;

  // Actions
  setSettings: (settings: Settings) => void;
  addLogItem: (item: LogItem) => void;
  addActivity: (item: ActivityItem) => void;
  setServices: (serviceMap: Map<string, Service>) => void;
  removeService: (serviceId: ServiceId) => void;
  clearLogs: () => void;
  clearActivity: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  // Initial state
  settings: {
    wavs_home: null,
    saved_registries: [],
    saved_service_managers: [],
    saved_services: [],
    mcp_enabled: false,
    mcp_auto_start: false,
    mcp_token: null,
    env_vars: {},
    agent_model_provider: null,
    agent_model_id: null,
    agent_thinking_level: null,
    agent_auto_start: false,
    agent_panel_width: null,
  },
  logList: [],
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

  addActivity: (item) =>
    set((state) => {
      const next = [...state.activityList, item];
      if (next.length > MAX_ACTIVITY_ITEMS) {
        // ERR-02: Failed events are never auto-removed from the activity feed.
        // Evict oldest non-failed items first; failed items are preserved.
        const evictable: ActivityItem[] = [];
        const preserved: ActivityItem[] = [];
        for (const entry of next) {
          if (entry.kind === 'submission_failed') {
            preserved.push(entry);
          } else {
            evictable.push(entry);
          }
        }
        const trimmed = evictable.length > MAX_ACTIVITY_ITEMS
          ? evictable.slice(evictable.length - MAX_ACTIVITY_ITEMS)
          : evictable;
        // Merge and maintain insertion order by id
        const merged = [...trimmed, ...preserved].sort((a, b) => a.id - b.id);
        return { activityList: merged };
      }
      return { activityList: next };
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

  clearActivity: () => set({ activityList: [] }),
}));
