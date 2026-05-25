import { invoke } from '@tauri-apps/api/core';
import type {
  Settings,
  SavedRegistry,
  DirectoryChooserResponse,
  ChainConfigs,
  Service,
  ServiceManager,
  HealthStatus,
  IpfsProvider,
  ComponentDigestResult,
  McpStatus,
  KvEntry,
  FsEntry,
} from '../types';

export async function setWavsHome(): Promise<string | null> {
  const resp = await invoke<DirectoryChooserResponse>('cmd_set_wavs_home');

  if ('selected' in resp) {
    return resp.selected;
  }
  return null;
}

export async function pickFolder(): Promise<string | null> {
  const resp = await invoke<DirectoryChooserResponse>('cmd_pick_folder');
  if ('selected' in resp) {
    return resp.selected;
  }
  return null;
}

export async function getSettings(): Promise<Settings> {
  return invoke<Settings>('cmd_get_settings');
}

export async function savePoaRegistries(registries: SavedRegistry[]): Promise<void> {
  return invoke<void>('cmd_save_poa_registries', { registries });
}

export async function restart(): Promise<void> {
  return invoke<void>('cmd_restart');
}

export async function startWavs(): Promise<void> {
  return invoke<void>('cmd_start_wavs');
}

export async function getChainConfigs(): Promise<ChainConfigs> {
  return invoke<ChainConfigs>('cmd_get_chain_configs');
}

export async function getServices(): Promise<Service[]> {
  return invoke<Service[]>('cmd_get_services');
}

export async function addService(manager: ServiceManager): Promise<Service> {
  return invoke<Service>('cmd_add_service', { manager });
}

export async function removeService(manager: ServiceManager): Promise<void> {
  return invoke<void>('cmd_remove_service', { manager });
}

export async function saveServiceToNode(serviceJson: string): Promise<string> {
  return invoke<string>('cmd_save_service_to_node', { service_json: serviceJson });
}

// Keychain commands
export async function hasMnemonic(): Promise<boolean> {
  return invoke<boolean>('cmd_has_mnemonic');
}

export async function storeMnemonic(mnemonic: string): Promise<void> {
  return invoke<void>('cmd_store_mnemonic', { mnemonic });
}

export async function getMnemonic(): Promise<string> {
  return invoke<string>('cmd_get_mnemonic');
}

export async function deleteMnemonic(): Promise<void> {
  return invoke<void>('cmd_delete_mnemonic');
}

// Health commands
export async function getHealthStatus(): Promise<HealthStatus> {
  return invoke<HealthStatus>('cmd_get_health_status');
}

// TOML commands
export async function readWavsToml(): Promise<string> {
  return invoke<string>('cmd_read_wavs_toml');
}

export async function writeWavsToml(content: string): Promise<void> {
  return invoke<void>('cmd_write_wavs_toml', { content });
}

// IPFS commands
export async function uploadToIpfs(content: string, provider: IpfsProvider): Promise<string> {
  return invoke<string>('cmd_upload_to_ipfs', { content, provider });
}

// Component commands
export async function getComponentDigest(
  domain: string | null,
  packageName: string,
  version: string | null
): Promise<ComponentDigestResult> {
  return invoke<ComponentDigestResult>('cmd_get_component_digest', {
    domain,
    package: packageName,
    version,
  });
}

export async function publishComponent(filePath: string): Promise<string> {
  return invoke<string>('cmd_publish_component', { file_path: filePath });
}

// MCP server commands
export async function getMcpBinaryPath(): Promise<string | null> {
  return invoke<string | null>('cmd_get_mcp_binary_path');
}

export async function getWavsUrl(): Promise<string> {
  return invoke<string>('cmd_get_wavs_url');
}

export async function startMcpServer(): Promise<void> {
  return invoke<void>('cmd_start_mcp_server');
}

export async function stopMcpServer(): Promise<void> {
  return invoke<void>('cmd_stop_mcp_server');
}

export async function getMcpStatus(): Promise<McpStatus> {
  return invoke<McpStatus>('cmd_get_mcp_status');
}

export async function saveMcpSettings(
  mcpAutoStart: boolean,
  mcpToken: string | null
): Promise<void> {
  return invoke<void>('cmd_save_mcp_settings', {
    mcp_auto_start: mcpAutoStart,
    mcp_token: mcpToken,
  });
}

export async function clearPersistedServices(): Promise<void> {
  return invoke<void>('cmd_clear_persisted_services');
}

export async function saveEnvVars(envVars: Record<string, string>): Promise<void> {
  return invoke<void>('cmd_save_env_vars', { env_vars: envVars });
}

export async function registerClaudeMcp(projectPath: string): Promise<string> {
  return invoke<string>('cmd_register_claude_mcp', { project_path: projectPath });
}

// Storage commands (KV store + filesystem)
export async function listKvEntries(serviceId: string): Promise<KvEntry[]> {
  return invoke<KvEntry[]>('cmd_list_kv_entries', { service_id: serviceId });
}

export async function listFsEntries(serviceId: string, path: string): Promise<FsEntry[]> {
  return invoke<FsEntry[]>('cmd_list_fs_entries', { service_id: serviceId, path });
}

export async function readFsFile(serviceId: string, path: string): Promise<number[]> {
  return invoke<number[]>('cmd_read_fs_file', { service_id: serviceId, path });
}
