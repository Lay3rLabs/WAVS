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
  saved_services: Service[];
  mcp_enabled: boolean;
  mcp_auto_start: boolean;
  mcp_token: string | null;
  env_vars: Record<string, string>;
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
  | { Service: string }
  | { Keychain: string };

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
  trigger_data: TriggerData;
  tx_hash: string | null;
}

export interface SubmissionErrorEvent {
  service_id: ServiceId;
  workflow_id: WorkflowId;
  trigger_data: TriggerData;
  error_message: string;
}

export type ServiceAction = 'added' | 'removed' | 'paused' | 'resumed';

export interface ServiceEvent {
  action: ServiceAction;
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
  | { download: { uri: string; digest: string } }
  | { registry: { digest: string; domain: string | null; version: string | null; package: string } }
  | { digest: string };

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
  | { cosmos_contract_event: { address: string; chain: ChainKey; event_type: string } }
  | { evm_contract_event: { address: string; chain: ChainKey; event_hash: string } }
  | { block_interval: { chain: ChainKey; n_blocks: number; start_block: number | null; end_block: number | null } }
  | { cron: { schedule: string; start_time: number | null; end_time: number | null } }
  | { at_proto_event: { collection: string; repo_did: string | null; action: AtProtoAction | null } }
  | { hypercore_append: { feed_key: string } }
  | 'manual';

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
  | { HypercoreAppend: { feed_key: string; index: number; data: number[] } }
  | { Raw: number[] };

// Submit types
export type Submit =
  | 'none'
  | { aggregator: { component: Component; signature_kind: SignatureKind } };

export interface SignatureKind {
  algorithm: SignatureAlgorithm;
  prefix: SignaturePrefix | null;
}

export type SignatureAlgorithm = 'secp256k1' | 'bls12381';
export type SignaturePrefix = 'eip191' | 'none';

// P2P network status (mirrors packages/types/src/http.rs:134 P2pStatus)
export interface P2pStatus {
  enabled: boolean;
  discovery_mode: string;
  local_peer_id: string | null;
  listen_addresses: string[];
  connected_peers: number;
  peer_ids: string[];
  subscribed_services: string[];
  peer_subscriptions: Record<string, number>;  // OBS-01: per-service peer counts
}

// Service signer response (mirrors packages/types/src/http.rs:13 SignerResponse)
// Rust uses #[serde(rename_all = "snake_case")] with NO tag attribute = externally tagged enum
// JSON output: {"secp256k1": {"hd_index": 0, "evm_address": "0x..."}} or {"bls12381": {"hd_index": 0, "g1_pubkey_hex": "..."}}
export type SignerResponse =
  | { secp256k1: { hd_index: number; evm_address: string } }
  | { bls12381: { hd_index: number; g1_pubkey_hex: string } };

// BLS public key derivation response (for cmd_derive_bls_pubkey)
export interface BlsPubkeyResponse {
  g1_pubkey_hex: string;
}

// BLS proof-of-possession response (for cmd_bls_sign_proof_of_possession)
export interface BlsProofResponse {
  g1_pubkey_hex: string;
  g2_proof_hex: string;
}

// Chain types
export interface ChainConfigs {
  cosmos: Record<string, CosmosChainConfigBuilder>;
  evm: Record<string, EvmChainConfigBuilder>;
  dev: Record<string, AnyChainConfig>;
}

// Rust uses #[serde(tag = "type", rename_all = "snake_case")] -- internally tagged
export type AnyChainConfig =
  | ({ type: 'cosmos' } & CosmosChainConfig)
  | ({ type: 'evm' } & EvmChainConfig);

export interface CosmosChainConfigBuilder {
  bech32_prefix: string;
  rpc_endpoint: string | null;
  grpc_endpoint: string | null;
  gas_price: number;
  gas_denom: string;
  faucet_endpoint: string | null;
}

export interface EvmChainConfigBuilder {
  ws_endpoints: string[];
  http_endpoint: string | null;
  faucet_endpoint: string | null;
  ws_priority_endpoint_index: number | null;
}

export interface CosmosChainConfig {
  chain_id: string;
  bech32_prefix: string;
  rpc_endpoint: string | null;
  grpc_endpoint: string | null;
  gas_price: number;
  gas_denom: string;
  faucet_endpoint: string | null;
}

export interface EvmChainConfig {
  chain_id: string;
  ws_endpoints: string[];
  http_endpoint: string | null;
  faucet_endpoint: string | null;
  ws_priority_endpoint_index: number | null;
}

// Service Manager types (for adding services)
export type ServiceManager =
  | { evm: { chain: ChainKey; address: string } }
  | { cosmos: { chain: ChainKey; address: string } };

// Envelope type (simplified, used for display)
export type Envelope = unknown;

// IPFS types
export type IpfsProvider =
  | { local: { api_url: string } }
  | { pinata: { api_key: string } };

// Component digest result
export interface ComponentDigestResult {
  digest: string;
  resolved_version: string;
}

// MCP server types
export interface McpStatus {
  running: boolean;
  pid: number | null;
}

// Storage types (KV store and filesystem)
export interface KvEntry {
  bucket: string;
  key: string;
  value_b64: string;
}

export interface FsEntry {
  name: string;
  is_dir: boolean;
  size: number | null;
}

// Activity types (unified triggers + submissions)
export type ActivityStatus = 'pending' | 'executed' | 'confirmed' | 'error';

export interface UnifiedActivity {
  id: number;
  correlationKey: string;
  triggerTs: number;
  submissionTs: number | null;
  status: ActivityStatus;
  serviceId: ServiceId;
  workflowId: WorkflowId;
  triggerData: TriggerData;
  triggerConfig?: TriggerConfig;
  txHash: string | null;
  errorMessage: string | null;
}

export function correlationKey(serviceId: string, workflowId: string, triggerData: TriggerData): string {
  if ('EvmContractEvent' in triggerData) {
    const d = triggerData.EvmContractEvent;
    return `${serviceId}:${workflowId}:evm:${d.chain}:${d.block_number}:${d.log_index}`;
  }
  if ('CosmosContractEvent' in triggerData) {
    const d = triggerData.CosmosContractEvent;
    return `${serviceId}:${workflowId}:cosmos:${d.chain}:${d.block_height}:${d.event_index}`;
  }
  if ('BlockInterval' in triggerData) {
    const d = triggerData.BlockInterval;
    return `${serviceId}:${workflowId}:block:${d.chain}:${d.block_height}`;
  }
  if ('Cron' in triggerData) {
    return `${serviceId}:${workflowId}:cron:${triggerData.Cron.trigger_time}`;
  }
  if ('AtProtoEvent' in triggerData) {
    const d = triggerData.AtProtoEvent;
    return `${serviceId}:${workflowId}:atproto:${d.sequence}`;
  }
  if ('HypercoreAppend' in triggerData) {
    const d = triggerData.HypercoreAppend;
    return `${serviceId}:${workflowId}:hypercore:${d.feed_key}:${d.index}`;
  }
  if ('Raw' in triggerData) {
    return `${serviceId}:${workflowId}:raw:${triggerData.Raw.length}:${Date.now()}`;
  }
  return `${serviceId}:${workflowId}:unknown:${Date.now()}`;
}

/** Check if a service uses BLS signature algorithm in any workflow */
export function isBLSService(service: Service): boolean {
  return Object.values(service.workflows).some((wf) => {
    if (wf.submit === 'none') return false;
    return wf.submit.aggregator.signature_kind.algorithm === 'bls12381';
  });
}

// Helper to get a human-readable service key from manager (for display/fallback only)
export function getServiceKey(service: Service): string {
  const manager = service.manager;
  if ('evm' in manager) {
    return `evm:${manager.evm.chain}:${manager.evm.address}`;
  }
  if ('cosmos' in manager) {
    return `cosmos:${manager.cosmos.chain}:${manager.cosmos.address}`;
  }
  return 'unknown';
}

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.replace(/^0x/i, '');
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

// Compute the same ServiceId hash as the Rust backend:
// SHA256(prefix_bytes + chain_string_bytes + address_bytes)
export async function computeServiceHash(manager: ServiceManager): Promise<string> {
  const encoder = new TextEncoder();
  const parts: Uint8Array[] = [];

  if ('evm' in manager) {
    parts.push(encoder.encode('evm'));
    parts.push(encoder.encode(manager.evm.chain));
    // Rust uses address.as_slice() which gives raw 20 bytes
    parts.push(hexToBytes(manager.evm.address));
  } else if ('cosmos' in manager) {
    parts.push(encoder.encode('cosmos'));
    parts.push(encoder.encode(manager.cosmos.chain));
    // Rust uses address.to_vec() -- UTF-8 bytes of the address string
    parts.push(encoder.encode(manager.cosmos.address));
  }

  const totalLen = parts.reduce((sum, p) => sum + p.length, 0);
  const combined = new Uint8Array(totalLen);
  let offset = 0;
  for (const part of parts) {
    combined.set(part, offset);
    offset += part.length;
  }

  const hash = await crypto.subtle.digest('SHA-256', combined);
  return bytesToHex(new Uint8Array(hash));
}

// Build a Map<hashId, Service> from a list of services
export async function buildServiceMap(
  services: Service[],
): Promise<Map<string, Service>> {
  const map = new Map<string, Service>();
  for (const service of services) {
    const hashId = await computeServiceHash(service.manager);
    map.set(hashId, service);
  }
  return map;
}

// Helper to get trigger label
export function getTriggerLabel(trigger: Trigger): string {
  if (trigger === 'manual') return 'Manual';
  if ('cosmos_contract_event' in trigger) return 'Cosmos Contract Event';
  if ('evm_contract_event' in trigger) return 'EVM Contract Event';
  if ('block_interval' in trigger) return 'Block Interval';
  if ('cron' in trigger) return 'Cron';
  if ('at_proto_event' in trigger) return 'AtProto Event';
  if ('hypercore_append' in trigger) return 'Hypercore Append';
  return 'Unknown';
}

// Helper to get trigger data label
export function getTriggerDataLabel(data: TriggerData): string {
  if ('CosmosContractEvent' in data) return 'Cosmos Contract Event';
  if ('EvmContractEvent' in data) return 'EVM Contract Event';
  if ('BlockInterval' in data) return 'Block Interval';
  if ('Cron' in data) return 'Cron';
  if ('AtProtoEvent' in data) return 'AtProto Event';
  if ('HypercoreAppend' in data) return 'Hypercore Append';
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
