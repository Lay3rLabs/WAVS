import { create } from 'zustand';
import type {
  Service,
  Workflow,
  Trigger,
  Component,
  Submit,
  ComponentSource,
  AllowedHostPermission,
  ServiceManager,
} from '../types';

export type BuilderStep = 'contract' | 'basics' | 'workflows' | 'review' | 'deploy';

export interface WorkflowDraft {
  id: string;
  trigger: Trigger | null;
  component: ComponentDraft;
  submit: SubmitDraft;
}

export interface ComponentDraft {
  sourceType: 'registry' | 'download' | 'digest';
  // Registry fields
  domain: string;
  package: string;
  version: string;
  // Download fields
  uri: string;
  // Shared
  digest: string;
  // Permissions
  httpHosts: 'all' | 'none' | 'specific';
  specificHosts: string[];
  fileSystem: boolean;
  // Limits
  fuelLimit: string;
  timeLimitSeconds: string;
  // Config
  config: Record<string, string>;
  envKeys: string[];
}

export interface SubmitDraft {
  type: 'none' | 'aggregator';
  aggregatorUrl: string;
  component: ComponentDraft;
  signatureAlgorithm: 'secp256k1';
  signaturePrefix: 'eip191' | 'none';
}

function createDefaultComponent(): ComponentDraft {
  return {
    sourceType: 'registry',
    domain: '',
    package: '',
    version: '',
    uri: '',
    digest: '',
    httpHosts: 'none',
    specificHosts: [],
    fileSystem: false,
    fuelLimit: '',
    timeLimitSeconds: '',
    config: {},
    envKeys: [],
  };
}

function createDefaultSubmit(): SubmitDraft {
  return {
    type: 'none',
    aggregatorUrl: '',
    component: createDefaultComponent(),
    signatureAlgorithm: 'secp256k1',
    signaturePrefix: 'eip191',
  };
}

function createWorkflowDraft(): WorkflowDraft {
  return {
    id: crypto.randomUUID(),
    trigger: null,
    component: createDefaultComponent(),
    submit: createDefaultSubmit(),
  };
}

export type DeployStepStatus = 'pending' | 'in_progress' | 'done' | 'error';

export interface DeployState {
  ipfsStatus: DeployStepStatus;
  setUriStatus: DeployStepStatus;
  registerStatus: DeployStepStatus;
  cid: string | null;
  error: string | null;
}

interface ServiceBuilderState {
  step: BuilderStep;
  name: string;
  selectedRegistryKey: string | null;

  // Manual manager input (fallback when no registry selected)
  manualChain: string;
  manualAddress: string;

  workflows: WorkflowDraft[];

  // Deploy config
  ipfsProvider: 'local' | 'pinata';
  localIpfsUrl: string;
  pinataApiKey: string;

  // Deploy state
  deploying: boolean;
  deployState: DeployState;

  // Actions
  setStep: (step: BuilderStep) => void;
  setName: (name: string) => void;
  setSelectedRegistry: (key: string | null) => void;
  setManualChain: (chain: string) => void;
  setManualAddress: (address: string) => void;
  addWorkflow: () => void;
  removeWorkflow: (id: string) => void;
  updateWorkflow: (id: string, updates: Partial<WorkflowDraft>) => void;
  setIpfsProvider: (provider: 'local' | 'pinata') => void;
  setLocalIpfsUrl: (url: string) => void;
  setPinataApiKey: (key: string) => void;
  setDeploying: (deploying: boolean) => void;
  setDeployState: (state: Partial<DeployState>) => void;
  reset: () => void;

  // Computed
  buildServiceJson: () => Service | null;
  getServiceManager: () => ServiceManager | null;
}

function buildComponent(draft: ComponentDraft): Component | null {
  let source: ComponentSource;
  switch (draft.sourceType) {
    case 'registry':
      if (!draft.package || !draft.digest) return null;
      source = {
        registry: {
          digest: draft.digest,
          domain: draft.domain || null,
          version: draft.version || null,
          package: draft.package,
        },
      };
      break;
    case 'download':
      if (!draft.uri || !draft.digest) return null;
      source = { download: { uri: draft.uri, digest: draft.digest } };
      break;
    case 'digest':
      if (!draft.digest) return null;
      source = { digest: draft.digest };
      break;
    default:
      return null;
  }

  let allowed_http_hosts: AllowedHostPermission;
  if (draft.httpHosts === 'all') {
    allowed_http_hosts = 'all';
  } else if (draft.httpHosts === 'specific' && draft.specificHosts.length > 0) {
    allowed_http_hosts = { only: draft.specificHosts };
  } else {
    allowed_http_hosts = 'none';
  }

  return {
    source,
    permissions: {
      allowed_http_hosts,
      file_system: draft.fileSystem,
      raw_sockets: false,
      dns_resolution: false,
    },
    fuel_limit: draft.fuelLimit ? parseInt(draft.fuelLimit, 10) : null,
    time_limit_seconds: draft.timeLimitSeconds ? parseInt(draft.timeLimitSeconds, 10) : null,
    config: draft.config,
    env_keys: draft.envKeys,
  };
}

function buildSubmit(draft: SubmitDraft): Submit | null {
  if (draft.type === 'none') return 'none';

  const component = buildComponent(draft.component);
  if (!component || !draft.aggregatorUrl) return null;

  return {
    aggregator: {
      url: draft.aggregatorUrl,
      component,
      signature_kind: {
        algorithm: draft.signatureAlgorithm,
        prefix: draft.signaturePrefix === 'none' ? null : draft.signaturePrefix,
      },
    },
  };
}

const initialDeployState: DeployState = {
  ipfsStatus: 'pending',
  setUriStatus: 'pending',
  registerStatus: 'pending',
  cid: null,
  error: null,
};

export const useServiceBuilderStore = create<ServiceBuilderState>((set, get) => ({
  step: 'contract',
  name: '',
  selectedRegistryKey: null,
  manualChain: '',
  manualAddress: '',
  workflows: [createWorkflowDraft()],
  ipfsProvider: 'local',
  localIpfsUrl: 'http://127.0.0.1:5001',
  pinataApiKey: '',
  deploying: false,
  deployState: { ...initialDeployState },

  setStep: (step) => set({ step }),
  setName: (name) => set({ name }),
  setSelectedRegistry: (key) => set({ selectedRegistryKey: key }),
  setManualChain: (chain) => set({ manualChain: chain }),
  setManualAddress: (address) => set({ manualAddress: address }),

  addWorkflow: () =>
    set((state) => ({
      workflows: [...state.workflows, createWorkflowDraft()],
    })),

  removeWorkflow: (id) =>
    set((state) => ({
      workflows: state.workflows.filter((w) => w.id !== id),
    })),

  updateWorkflow: (id, updates) =>
    set((state) => ({
      workflows: state.workflows.map((w) =>
        w.id === id ? { ...w, ...updates } : w
      ),
    })),

  setIpfsProvider: (provider) => set({ ipfsProvider: provider }),
  setLocalIpfsUrl: (url) => set({ localIpfsUrl: url }),
  setPinataApiKey: (key) => set({ pinataApiKey: key }),
  setDeploying: (deploying) => set({ deploying }),
  setDeployState: (partial) =>
    set((state) => ({
      deployState: { ...state.deployState, ...partial },
    })),

  reset: () =>
    set({
      step: 'contract',
      name: '',
      selectedRegistryKey: null,
      manualChain: '',
      manualAddress: '',
      workflows: [createWorkflowDraft()],
      ipfsProvider: 'local',
      localIpfsUrl: 'http://127.0.0.1:5001',
      pinataApiKey: '',
      deploying: false,
      deployState: { ...initialDeployState },
    }),

  getServiceManager: () => {
    // This will be resolved from the registry or manual input by the UI layer
    // Here we return null since registry resolution needs the POA store
    return null;
  },

  buildServiceJson: () => {
    const state = get();
    if (!state.name) return null;

    const workflows: Record<string, Workflow> = {};
    for (const draft of state.workflows) {
      if (!draft.trigger) return null;

      const component = buildComponent(draft.component);
      if (!component) return null;

      const submit = buildSubmit(draft.submit);
      if (submit === null) return null;

      workflows[draft.id] = {
        trigger: draft.trigger,
        component,
        submit,
      };
    }

    if (Object.keys(workflows).length === 0) return null;

    // Manager will be set at deploy time from the selected registry
    const service: Service = {
      name: state.name,
      workflows,
      status: 'active',
      manager: { evm: { chain: '', address: '' } }, // placeholder, resolved at deploy
    };

    return service;
  },
}));
