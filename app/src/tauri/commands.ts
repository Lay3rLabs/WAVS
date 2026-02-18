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
} from '../types';

export async function setWavsHome(): Promise<string | null> {
  const resp = await invoke<DirectoryChooserResponse>('cmd_set_wavs_home');

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
