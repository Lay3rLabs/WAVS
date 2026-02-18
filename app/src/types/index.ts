// Settings types
export interface SavedRegistry {
  chain_id: number;
  chain_key: string;
  rpc_url: string;
  address: string;
}

export interface Settings {
  wavs_home: string | null;
  saved_registries: SavedRegistry[];
  saved_service_managers: ServiceManager[];
}

// Health types
// Rust enum serializes as: "healthy" (string) or { "unhealthy": { "error": "..." } }
export type ChainHealthResult =
  | 'healthy'
  | { unhealthy: { error: string } };

export interface HealthStatus {
  timestamp: number;
  chains: Record<ChainKey, ChainHealthResult>;
}

export function isChainHealthy(result: ChainHealthResult): boolean {
  return result === 'healthy';
}

export function getChainError(result: ChainHealthResult): string | null {
  if (typeof result === 'object' && 'unhealthy' in result) {
    return result.unhealthy.error;
  }
  return null;
}

// Command types
export type DirectoryChooserResponse =
  | { none: null }
  | { selected: string };

// Error types
export type AppError =
  | { Io: string }
  | { Json: string }
  | { Toml: string }
  | { Settings: string }
  | { EventEmitter: string }
  | { Tauri: string }
  | { WavsConfig: string }
  | { HealthCheck: string }
  | { MissingChain: string }
  | { WavsNotRunning: null }
  | { Service: string };

export function getErrorMessage(err: unknown): string {
  if (typeof err === 'string') return err;
  if (err instanceof Error) return err.message;
  if (typeof err === 'object' && err !== null) {
    // Handle AppError variants like { Service: "message" }
    const values = Object.values(err);
    if (values.length > 0 && typeof values[0] === 'string') {
      return values[0];
    }
    // Fallback to JSON
    return JSON.stringify(err);
  }
  return String(err);
}

// Log types
export type LogLevel = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE';

export interface LogItem {
  ts: number; // timestamp in milliseconds
  level: LogLevel;
  target: string;
  fields: string;
}

// Event types
export interface SettingsEvent {
  settings: Settings;
}

export interface LogEvent {
  level: string;
  target: string;
  fields: string;
}

export interface TriggerEvent {
  action: TriggerAction;
}

export interface SubmissionEvent {
  service_id: ServiceId;
  workflow_id: WorkflowId;
  envelope: Envelope;
  trigger_data: TriggerData;
  submit: Submit;
}

// Service types
export type ServiceId = string;
export type WorkflowId = string;
export type ChainKey = string;

export type ServiceStatus = 'active' | 'paused';

export interface Service {
  name: string;
  workflows: Record<WorkflowId, Workflow>;
  status: ServiceStatus;
  manager: ServiceManager;
}

export interface Workflow {
  trigger: Trigger;
  component: Component;
  submit: Submit;
}

export interface Component {
  source: ComponentSource;
  permissions: Permissions;
  fuel_limit: number | null;
  time_limit_seconds: number | null;
  config: Record<string, string>;
  env_keys: string[];
}

export type ComponentSource =
  | { Download: { uri: string; digest: string } }
  | { Registry: { digest: string; domain: string | null; version: string | null; package: string } }
  | { Digest: string };

export interface Permissions {
  allowed_http_hosts: AllowedHostPermission;
  file_system: boolean;
  raw_sockets: boolean;
  dns_resolution: boolean;
}

export type AllowedHostPermission =
  | 'all'
  | { only: string[] }
  | 'none';

// Trigger types
export type Trigger =
  | { CosmosContractEvent: { address: string; chain: ChainKey; event_type: string } }
  | { EvmContractEvent: { address: string; chain: ChainKey; event_hash: string } }
  | { BlockInterval: { chain: ChainKey; n_blocks: number; start_block: number | null; end_block: number | null } }
  | { Cron: { schedule: string; start_time: number | null; end_time: number | null } }
  | { AtProtoEvent: { collection: string; repo_did: string | null; action: AtProtoAction | null } }
  | 'Manual';

export type AtProtoAction = 'create' | 'update' | 'delete';

export interface TriggerConfig {
  service_id: ServiceId;
  workflow_id: WorkflowId;
  trigger: Trigger;
}

export interface TriggerAction {
  config: TriggerConfig;
  data: TriggerData;
}

export type TriggerData =
  | { CosmosContractEvent: { contract_address: string; chain: ChainKey; event: unknown; block_height: number; event_index: number } }
  | { EvmContractEvent: { chain: ChainKey; contract_address: string; log_data: unknown; tx_hash: string; block_number: number; log_index: number; block_hash: string; block_timestamp: number | null; tx_index: number } }
  | { BlockInterval: { chain: ChainKey; block_height: number } }
  | { Cron: { trigger_time: number } }
  | { AtProtoEvent: { sequence: number; timestamp: number; repo: string; collection: string; rkey: string; action: AtProtoAction; cid: string | null; record: unknown | null; rev: string | null; op_index: number | null } }
  | { Raw: number[] };

// Submit types
export type Submit =
  | 'None'
  | { Aggregator: { url: string; component: Component; signature_kind: SignatureKind } };

export interface SignatureKind {
  algorithm: SignatureAlgorithm;
  prefix: SignaturePrefix | null;
}

export type SignatureAlgorithm = 'secp256k1';
export type SignaturePrefix = 'eip191';

// Chain types
export interface ChainConfigs {
  cosmos: Record<string, CosmosChainConfigBuilder>;
  evm: Record<string, EvmChainConfigBuilder>;
  dev: Record<string, AnyChainConfig>;
}

export type AnyChainConfig =
  | { Cosmos: CosmosChainConfig }
  | { Evm: EvmChainConfig };

export interface CosmosChainConfigBuilder {
  chain_id: string | null;
  rpc_endpoint: string | null;
  grpc_endpoint: string | null;
  address_kind: string | null;
  gas_denom: string | null;
  gas_adjustment: number | null;
  min_gas_price: string | null;
}

export interface EvmChainConfigBuilder {
  chain_id: number | null;
  rpc_endpoint: string | null;
  ws_endpoint: string | null;
}

export interface CosmosChainConfig {
  name: string;
  chain_id: string;
  rpc_endpoint: string;
  grpc_endpoint: string | null;
  address_kind: string;
  gas_denom: string;
  gas_adjustment: number;
  min_gas_price: string;
}

export interface EvmChainConfig {
  name: string;
  chain_id: number;
  rpc_endpoint: string;
  ws_endpoint: string | null;
}

// Service Manager types (for adding services)
export type ServiceManager =
  | { evm: { chain: ChainKey; address: string } }
  | { cosmos: { chain: ChainKey; address: string } };

// Envelope type (simplified, used for display)
export type Envelope = unknown;

// Helper to get service ID from manager
export function getServiceId(service: Service): string {
  const manager = service.manager;
  if ('evm' in manager) {
    return `evm:${manager.evm.chain}:${manager.evm.address}`;
  }
  if ('cosmos' in manager) {
    return `cosmos:${manager.cosmos.chain}:${manager.cosmos.address}`;
  }
  return 'unknown';
}

// Helper to get trigger label
export function getTriggerLabel(trigger: Trigger): string {
  if (trigger === 'Manual') return 'Manual';
  if ('CosmosContractEvent' in trigger) return 'Cosmos Contract Event';
  if ('EvmContractEvent' in trigger) return 'EVM Contract Event';
  if ('BlockInterval' in trigger) return 'Block Interval';
  if ('Cron' in trigger) return 'Cron';
  if ('AtProtoEvent' in trigger) return 'AtProto Event';
  return 'Unknown';
}

// Helper to get trigger data label
export function getTriggerDataLabel(data: TriggerData): string {
  if ('CosmosContractEvent' in data) return 'Cosmos Contract Event';
  if ('EvmContractEvent' in data) return 'EVM Contract Event';
  if ('BlockInterval' in data) return 'Block Interval';
  if ('Cron' in data) return 'Cron';
  if ('AtProtoEvent' in data) return 'AtProto Event';
  if ('Raw' in data) return 'Raw';
  return 'Unknown';
}

// Helper to get chain from service manager
export function getServiceChain(manager: ServiceManager): ChainKey {
  if ('evm' in manager) return manager.evm.chain;
  if ('cosmos' in manager) return manager.cosmos.chain;
  return 'unknown';
}

// Helper to get address from service manager
export function getServiceAddress(manager: ServiceManager): string {
  if ('evm' in manager) return manager.evm.address;
  if ('cosmos' in manager) return manager.cosmos.address;
  return 'unknown';
}
