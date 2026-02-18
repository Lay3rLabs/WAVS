import { create } from 'zustand';
import type {
  Settings,
  LogItem,
  TriggerAction,
  SubmissionEvent,
  Service,
  ServiceId,
} from '../types';
import { getServiceId } from '../types';

interface AppState {
  // State
  settings: Settings;
  logList: LogItem[];
  triggersList: TriggerAction[];
  submissionsList: SubmissionEvent[];
  services: Map<ServiceId, Service>;

  // Computed
  isSettingsComplete: () => boolean;
  getServiceLabel: (serviceId: ServiceId) => string;

  // Actions
  setSettings: (settings: Settings) => void;
  addLogItem: (item: LogItem) => void;
  addTrigger: (trigger: TriggerAction) => void;
  addSubmission: (submission: SubmissionEvent) => void;
  setServices: (services: Service[]) => void;
  addService: (service: Service) => void;
  removeService: (serviceId: ServiceId) => void;
  clearLogs: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  // Initial state
  settings: { wavs_home: null, saved_registries: [], saved_service_managers: [] },
  logList: [],
  triggersList: [],
  submissionsList: [],
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
    set((state) => ({
      logList: [...state.logList, item],
    })),

  addTrigger: (trigger) =>
    set((state) => ({
      triggersList: [...state.triggersList, trigger],
    })),

  addSubmission: (submission) =>
    set((state) => ({
      submissionsList: [...state.submissionsList, submission],
    })),

  setServices: (services) =>
    set({
      services: new Map(services.map((s) => [getServiceId(s), s])),
    }),

  addService: (service) =>
    set((state) => {
      const newServices = new Map(state.services);
      newServices.set(getServiceId(service), service);
      return { services: newServices };
    }),

  removeService: (serviceId) =>
    set((state) => {
      const newServices = new Map(state.services);
      newServices.delete(serviceId);
      return { services: newServices };
    }),

  clearLogs: () => set({ logList: [] }),
}));
