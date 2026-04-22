use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use utils::{
    context::{AnyRuntime, AppContext},
    telemetry::{setup_metrics, Metrics},
    wkg::WkgClient,
};
use wavs::{config::HealthCheckMode, dispatcher::Dispatcher, health::SharedHealthStatus};
use wavs_gui_shared::{
    command::DirectoryChooserResponse,
    error::{AppError, AppResult},
    settings::{SavedRegistry, Settings},
};
use wavs_types::{ChainConfigs, Credential, Service, ServiceId, ServiceManager};

const KEYCHAIN_SERVICE: &str = "wavs-app";
const KEYCHAIN_ACCOUNT: &str = "mnemonic";

use wavs::health::HealthStatus;

use crate::state::{
    LogBufferState, McpServerState, MnemonicCacheState, SettingsState, WavsConfigState,
    WavsInstance, WavsInstanceState,
};

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_set_wavs_home(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    wavs_config: State<'_, WavsConfigState>,
) -> AppResult<DirectoryChooserResponse> {
    // Open native directory picker
    let directory = app.dialog().file().blocking_pick_folder();

    match directory {
        Some(dir) => {
            let path = dir.into_path().map_err(|e| AppError::Io(e.to_string()))?;
            wavs_config.reload(path.clone()).await?;

            settings
                .update(&app, |s| {
                    s.wavs_home = Some(path.clone());
                })
                .await?;

            Ok(DirectoryChooserResponse::Selected(path))
        }
        None => Ok(DirectoryChooserResponse::None),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_pick_folder(app: AppHandle) -> AppResult<DirectoryChooserResponse> {
    let directory = app.dialog().file().blocking_pick_folder();
    match directory {
        Some(dir) => {
            let path = dir.into_path().map_err(|e| AppError::Io(e.to_string()))?;
            Ok(DirectoryChooserResponse::Selected(path))
        }
        None => Ok(DirectoryChooserResponse::None),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_settings(settings: State<'_, SettingsState>) -> AppResult<Settings> {
    Ok(settings.get_cloned())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_save_poa_registries(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    registries: Vec<SavedRegistry>,
) -> AppResult<()> {
    settings
        .update(&app, |s| {
            s.saved_registries = registries.clone();
        })
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_restart(app: AppHandle) {
    tauri::process::restart(&app.env());
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_start_wavs(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    wavs_config: State<'_, WavsConfigState>,
    wavs_instance: State<'_, WavsInstanceState>,
    mnemonic_cache: State<'_, MnemonicCacheState>,
    mcp_state: State<'_, McpServerState>,
    log_buffer_state: State<'_, LogBufferState>,
) -> AppResult<()> {
    let mut config = match wavs_config.get_cloned() {
        Some(cfg) => cfg,
        None => {
            return Err(AppError::WavsConfig("missing".to_string()));
        }
    };

    // Set the signing mnemonic from the OS keychain
    if let Some(credential) = get_mnemonic_cached(&mnemonic_cache) {
        config.signing_mnemonic = Some(credential);
    }

    // Inject WAVS_ENV_* variables from settings into the process environment
    for (key, value) in &settings.get_cloned().env_vars {
        std::env::set_var(key, value);
    }

    let ctx = AppContext::new_with_runtime(AnyRuntime::TokioHandle(
        tauri::async_runtime::handle().inner().clone(),
    ));

    let health_status = SharedHealthStatus::new();

    let (chains, chain_configs) = {
        let chain_configs = config.chains.read().unwrap().clone();
        let chains = chain_configs.all_chain_keys().unwrap();
        (chains, chain_configs)
    };
    if !chains.is_empty() {
        match config.health_check_mode {
            HealthCheckMode::Bypass => {
                let health_status_clone = health_status.clone();
                ctx.rt.spawn(async move {
                    log::info!("Running health checks in background (bypass mode)");
                    health_status_clone.update(&chain_configs).await;
                    if health_status_clone.any_failing() {
                        log::warn!(
                            "Health check failed: {:#?}",
                            health_status_clone.read().unwrap()
                        );
                    }
                });
            }
            HealthCheckMode::Wait => {
                health_status.update(&chain_configs).await;
                if health_status.any_failing() {
                    log::warn!("Health check failed: {:#?}", health_status.read().unwrap());
                }
            }
            HealthCheckMode::Exit => {
                health_status.update(&chain_configs).await;
                if health_status.any_failing() {
                    return Err(AppError::HealthCheck(format!(
                        "Health check failed (exit mode): {:#?}",
                        health_status.read().unwrap()
                    )));
                }
            }
        }
    }

    let meter_provider = config.prometheus.as_ref().map(|collector| {
        setup_metrics(
            collector,
            "wavs_metrics",
            config.prometheus_push_interval_secs,
        )
    });
    let meter = opentelemetry::global::meter("wavs_metrics");
    let metrics = Metrics::new(meter);

    let dispatcher = Arc::new(
        Dispatcher::new(&config, metrics.wavs, app.clone())
            .map_err(|e| AppError::WavsConfig(e.to_string()))?,
    );

    // Restore saved services from the settings cache using the correct HD index from the
    // registry. We do NOT attempt a chain fetch here because the HTTP server hasn't started
    // yet (connection refused). Services without a local cache entry will be restored by
    // dispatcher.start() via service_registry.json once the HTTP server is bound.
    let saved_settings = settings.get_cloned();
    let saved_managers = saved_settings.saved_service_managers.clone();
    let saved_services = saved_settings.saved_services.clone();
    let hd_map = dispatcher.registry_hd_index_map();
    for manager in &saved_managers {
        if let Some(cached) = saved_services.iter().find(|s| &s.manager == manager) {
            let hd_index = hd_map.get(manager).copied();
            match dispatcher
                .add_service_direct(cached.clone(), hd_index)
                .await
            {
                Ok(_) => log::info!("Restored service from local cache: {:?}", manager),
                Err(e) => log::warn!("Failed to restore service from local cache: {}", e),
            }
        }
    }

    let log_buffer = log_buffer_state.inner.clone();
    let handle = std::thread::spawn({
        let ctx = ctx.clone();
        let dispatcher = dispatcher.clone();
        move || {
            wavs::run_server(
                ctx,
                config,
                dispatcher,
                metrics.http,
                health_status,
                log_buffer,
            )
        }
    });

    wavs_instance.set(WavsInstance {
        ctx,
        meter_provider,
        handle,
        dispatcher,
    });

    // Auto-start MCP server if configured
    if saved_settings.mcp_auto_start && !mcp_state.is_running() {
        if let Some(bin) = find_mcp_binary() {
            let wavs_url = match wavs_config.get_cloned() {
                Some(config) => format!("http://{}:{}", config.host, config.port),
                None => "http://localhost:8000".to_string(),
            };
            let mut cmd = std::process::Command::new(&bin);
            cmd.arg("--wavs-url").arg(&wavs_url);
            if let Some(token) = &saved_settings.mcp_token {
                cmd.arg("--token").arg(token);
            }
            // Inject chain credentials as env vars so wavs-mcp doesn't need to read
            // wavs.toml from the project directory (which may be a git repo).
            if let Some(wavs_home) = &saved_settings.wavs_home {
                let (cred, mnem) = read_wavs_home_credentials(wavs_home);
                if let Some(c) = cred {
                    cmd.env("WAVS_MCP_CHAIN_CREDENTIAL", c);
                }
                if let Some(m) = mnem {
                    cmd.env("WAVS_SIGNING_MNEMONIC", m);
                }
            }
            cmd.stdin(std::process::Stdio::piped());
            match cmd.spawn() {
                Ok(child) => {
                    mcp_state.set(child);
                    settings
                        .update(&app, |s| {
                            s.mcp_enabled = true;
                        })
                        .await
                        .ok();
                    log::info!("MCP server auto-started");
                }
                Err(e) => {
                    log::warn!("Failed to auto-start MCP server: {}", e);
                }
            }
        } else {
            log::warn!("MCP auto-start enabled but wavs-mcp binary not found");
        }
    }

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_chain_configs(
    wavs_config: State<'_, WavsConfigState>,
) -> AppResult<ChainConfigs> {
    Ok(wavs_config.chain_configs())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_services(
    wavs_instance: State<'_, WavsInstanceState>,
) -> AppResult<Vec<Service>> {
    wavs_instance
        .dispatcher()?
        .services
        .list(std::ops::Bound::Unbounded, std::ops::Bound::Unbounded)
        .map_err(|e| AppError::Service(e.to_string()))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_add_service(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    wavs_instance: State<'_, WavsInstanceState>,
    manager: ServiceManager,
) -> AppResult<Service> {
    let service = wavs_instance
        .dispatcher()?
        .add_service(manager.clone())
        .await
        .map_err(|e| AppError::Service(format!("Failed to add service: {}", e)))?;

    // Persist the manager and full service definition for restart recovery
    settings
        .update(&app, |s| {
            if !s.saved_service_managers.contains(&manager) {
                s.saved_service_managers.push(manager.clone());
            }
            s.saved_services.retain(|svc| svc.manager != manager);
            s.saved_services.push(service.clone());
        })
        .await?;

    Ok(service)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_remove_service(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    wavs_instance: State<'_, WavsInstanceState>,
    manager: ServiceManager,
) -> AppResult<()> {
    let service_id = ServiceId::from(&manager);
    wavs_instance
        .dispatcher()?
        .remove_service(service_id)
        .map_err(|e| AppError::Service(format!("Failed to remove service: {}", e)))?;
    settings
        .update(&app, |s| {
            s.saved_service_managers.retain(|m| m != &manager);
            s.saved_services.retain(|svc| svc.manager != manager);
        })
        .await?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_save_service_to_node(
    wavs_config: State<'_, WavsConfigState>,
    service_json: String,
) -> AppResult<String> {
    let wavs_url = match wavs_config.get_cloned() {
        Some(config) => format!("http://{}:{}", config.host, config.port),
        None => "http://localhost:8000".to_string(),
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/dev/services", wavs_url))
        .header("Content-Type", "application/json")
        .body(service_json)
        .send()
        .await
        .map_err(|e| AppError::Service(format!("Failed to save service to node: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Service(format!(
            "Node returned {}: {}",
            status, body
        )));
    }

    let save_resp: wavs_types::SaveServiceResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Service(format!("Failed to parse save response: {}", e)))?;

    Ok(format!("{}/dev/services/{}", wavs_url, save_resp.hash))
}

/// Load mnemonic from OS keyring and populate the cache.
fn load_from_keyring(cache: &MnemonicCacheState) -> Option<Credential> {
    let result = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .map(Credential::new);
    cache.set(result.clone());
    result
}

/// Return the cached mnemonic, or load from keyring on first access.
fn get_mnemonic_cached(cache: &MnemonicCacheState) -> Option<Credential> {
    if let Some(cached) = cache.get_cached() {
        return cached;
    }
    load_from_keyring(cache)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_has_mnemonic(cache: State<'_, MnemonicCacheState>) -> bool {
    get_mnemonic_cached(&cache).is_some()
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_store_mnemonic(cache: State<'_, MnemonicCacheState>, mnemonic: String) -> AppResult<()> {
    // Validate mnemonic format (basic check for word count)
    let word_count = mnemonic.split_whitespace().count();
    if word_count != 12 && word_count != 24 {
        return Err(AppError::Keychain(
            "Invalid mnemonic: must be 12 or 24 words".to_string(),
        ));
    }

    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    entry
        .set_password(&mnemonic)
        .map_err(|e| AppError::Keychain(e.to_string()))?;

    cache.set(Some(Credential::new(mnemonic)));
    log::info!("Mnemonic stored in keychain ({} words)", word_count);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_get_mnemonic(cache: State<'_, MnemonicCacheState>) -> AppResult<String> {
    get_mnemonic_cached(&cache)
        .map(|cred| cred.to_string())
        .ok_or_else(|| AppError::Keychain("No mnemonic found in keychain".to_string()))
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_delete_mnemonic(cache: State<'_, MnemonicCacheState>) -> AppResult<()> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| AppError::Keychain(e.to_string()))?;

    cache.clear();
    log::info!("Mnemonic deleted from keychain");
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_read_wavs_toml(settings: State<'_, SettingsState>) -> AppResult<String> {
    let wavs_home = settings
        .get_cloned()
        .wavs_home
        .ok_or_else(|| AppError::WavsConfig("wavs_home not set".to_string()))?;

    let path = wavs_home.join("wavs.toml");
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::Io(format!("Failed to read wavs.toml: {}", e)))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_write_wavs_toml(
    settings: State<'_, SettingsState>,
    wavs_config: State<'_, WavsConfigState>,
    content: String,
) -> AppResult<()> {
    // Validate TOML syntax
    content
        .parse::<toml::Table>()
        .map_err(|e| AppError::WavsConfig(format!("Invalid TOML: {}", e)))?;

    let wavs_home = settings
        .get_cloned()
        .wavs_home
        .ok_or_else(|| AppError::WavsConfig("wavs_home not set".to_string()))?;

    let path = wavs_home.join("wavs.toml");
    tokio::fs::write(&path, &content)
        .await
        .map_err(|e| AppError::Io(format!("Failed to write wavs.toml: {}", e)))?;

    // Reload config to pick up changes
    wavs_config.reload(wavs_home).await?;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_health_status(
    wavs_config: State<'_, WavsConfigState>,
) -> AppResult<HealthStatus> {
    let config = match wavs_config.get_cloned() {
        Some(cfg) => cfg,
        None => {
            return Err(AppError::WavsConfig("WAVS config not loaded".to_string()));
        }
    };

    let url = format!("http://{}:{}/health", config.host, config.port);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| AppError::HealthCheck(format!("Failed to connect to WAVS node: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::HealthCheck(format!(
            "WAVS node returned error status: {}",
            response.status()
        )));
    }

    let health_status: HealthStatus = response
        .json()
        .await
        .map_err(|e| AppError::HealthCheck(format!("Failed to parse health response: {}", e)))?;

    Ok(health_status)
}

// --- IPFS Upload ---

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpfsProvider {
    Local { api_url: String },
    Pinata { api_key: String },
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_upload_to_ipfs(content: String, provider: IpfsProvider) -> AppResult<String> {
    let client = reqwest::Client::new();

    match provider {
        IpfsProvider::Local { api_url } => {
            let url = format!("{}/api/v0/add?pin=true", api_url.trim_end_matches('/'));
            let part =
                reqwest::multipart::Part::bytes(content.into_bytes()).file_name("service.json");
            let form = reqwest::multipart::Form::new().part("file", part);

            let response = client
                .post(&url)
                .multipart(form)
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await
                .map_err(|e| AppError::Service(format!("IPFS upload failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Service(format!(
                    "IPFS upload returned {}: {}",
                    status, body
                )));
            }

            #[derive(Deserialize)]
            struct IpfsAddResponse {
                #[serde(rename = "Hash")]
                hash: String,
            }

            let resp: IpfsAddResponse = response
                .json()
                .await
                .map_err(|e| AppError::Service(format!("Failed to parse IPFS response: {}", e)))?;

            Ok(resp.hash)
        }
        IpfsProvider::Pinata { api_key } => {
            let part = reqwest::multipart::Part::bytes(content.into_bytes()).file_name(format!(
                "service-{}.json",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ));
            let form = reqwest::multipart::Form::new()
                .part("file", part)
                .text("network", "public");

            let response = client
                .post("https://uploads.pinata.cloud/v3/files")
                .bearer_auth(&api_key)
                .multipart(form)
                .timeout(std::time::Duration::from_secs(60))
                .send()
                .await
                .map_err(|e| AppError::Service(format!("Pinata upload failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Service(format!(
                    "Pinata upload returned {}: {}",
                    status, body
                )));
            }

            #[derive(Deserialize)]
            struct PinataData {
                cid: String,
            }
            #[derive(Deserialize)]
            struct PinataResponse {
                data: PinataData,
            }

            let resp: PinataResponse = response.json().await.map_err(|e| {
                AppError::Service(format!("Failed to parse Pinata response: {}", e))
            })?;

            Ok(resp.data.cid)
        }
    }
}

// --- Component Digest Lookup ---

#[derive(Serialize)]
pub struct ComponentDigestResult {
    pub digest: String,
    pub resolved_version: String,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_get_component_digest(
    domain: Option<String>,
    package: String,
    version: Option<String>,
) -> AppResult<ComponentDigestResult> {
    let default_domain = domain.clone().unwrap_or_else(|| "wa.dev".to_string());
    let wkg_client = WkgClient::new(default_domain)
        .map_err(|e| AppError::Service(format!("Failed to create Wkg client: {}", e)))?;

    let package_ref: wasm_pkg_client::PackageRef = package.parse().map_err(|e| {
        AppError::Service(format!("Invalid package reference '{}': {}", package, e))
    })?;

    let parsed_version = match &version {
        Some(v) => {
            let ver: wasm_pkg_client::Version = v
                .parse()
                .map_err(|e| AppError::Service(format!("Invalid version '{}': {}", v, e)))?;
            Some(ver)
        }
        None => None,
    };

    let (digest, resolved_version) = wkg_client
        .get_digest(domain, &package_ref, parsed_version.as_ref())
        .await
        .map_err(|e| AppError::Service(format!("Failed to get component digest: {}", e)))?;

    Ok(ComponentDigestResult {
        digest: digest.to_string(),
        resolved_version: resolved_version.to_string(),
    })
}

// --- Wasm Component Publish from File ---

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_publish_component(
    wavs_instance: State<'_, WavsInstanceState>,
    file_path: String,
) -> AppResult<String> {
    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| AppError::Io(format!("Failed to read wasm file '{}': {}", file_path, e)))?;

    let digest = wavs_instance
        .dispatcher()?
        .store_component_bytes(bytes)
        .map_err(|e| AppError::Service(format!("Failed to store component: {}", e)))?;

    Ok(digest.to_string())
}

// --- MCP Server ---

#[derive(Serialize)]
pub struct McpStatus {
    pub running: bool,
    pub pid: Option<u32>,
}

/// Resolve the wavs-mcp binary path.
/// Looks alongside the current executable first (bundled app), then checks both
/// debug and release profiles under the workspace target/ directory.
fn find_mcp_binary() -> Option<std::path::PathBuf> {
    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            // 1. Sibling of current executable (bundled app)
            let candidate = dir.join("wavs-mcp");
            if candidate.exists() {
                return Some(candidate);
            }

            // 2. Both profiles under target/ — handles dev app + release mcp (or vice versa).
            // current exe is at target/{debug,release}/<name>, so dir.parent() is target/.
            if let Some(target) = dir.parent() {
                for profile in &["release", "debug"] {
                    let candidate = target.join(profile).join("wavs-mcp");
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    None
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_get_mcp_binary_path() -> Option<String> {
    find_mcp_binary().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_get_wavs_url(wavs_config: State<'_, WavsConfigState>) -> String {
    match wavs_config.get_cloned() {
        Some(config) => format!("http://{}:{}", config.host, config.port),
        None => "http://localhost:8000".to_string(),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_start_mcp_server(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    wavs_config: State<'_, WavsConfigState>,
    mcp_state: State<'_, McpServerState>,
) -> AppResult<()> {
    if mcp_state.is_running() {
        return Ok(()); // already running
    }

    let s = settings.get_cloned();
    let wavs_url = match wavs_config.get_cloned() {
        Some(config) => format!("http://{}:{}", config.host, config.port),
        None => "http://localhost:8000".to_string(),
    };

    let bin = find_mcp_binary().ok_or_else(|| {
        AppError::Service(
            "wavs-mcp binary not found. Build it with: cargo build -p wavs-mcp".to_string(),
        )
    })?;

    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("--wavs-url").arg(&wavs_url);
    if let Some(token) = &s.mcp_token {
        cmd.arg("--token").arg(token);
    }

    // Pipe stdin so the MCP server doesn't immediately receive EOF (which would
    // cause the stdio transport to exit with "expect initialize request").
    // The Child holds the write end open; MCP clients that spawn their own
    // instance (Claude Desktop, Cursor) will handle their own stdio connection.
    cmd.stdin(std::process::Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| AppError::Service(format!("Failed to spawn wavs-mcp: {}", e)))?;

    mcp_state.set(child);

    // Persist mcp_enabled in settings
    settings
        .update(&app, |s| {
            s.mcp_enabled = true;
        })
        .await?;

    log::info!("MCP server started");
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_stop_mcp_server(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    mcp_state: State<'_, McpServerState>,
) -> AppResult<()> {
    mcp_state.stop();

    settings
        .update(&app, |s| {
            s.mcp_enabled = false;
        })
        .await?;

    log::info!("MCP server stopped");
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_get_mcp_status(mcp_state: State<'_, McpServerState>) -> McpStatus {
    McpStatus {
        running: mcp_state.is_running(),
        pid: mcp_state.pid(),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_save_mcp_settings(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    mcp_auto_start: bool,
    mcp_token: Option<String>,
) -> AppResult<()> {
    settings
        .update(&app, |s| {
            s.mcp_auto_start = mcp_auto_start;
            s.mcp_token = mcp_token.clone();
        })
        .await
}

// --- Environment Variables ---

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_save_env_vars(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    env_vars: HashMap<String, String>,
) -> AppResult<()> {
    // Apply to process environment immediately so running WAVS picks them up
    for (key, value) in &env_vars {
        std::env::set_var(key, value);
    }
    settings
        .update(&app, |s| {
            s.env_vars = env_vars.clone();
        })
        .await
}

// --- Register with Claude Code ---

/// Write wavs-mcp entry for `project_path` into ~/.claude.json.
/// Only writes command + args — credentials are stored in ~/.wavs/wavs.toml instead
/// so they work with all MCP clients, not just Claude Code.
fn register_claude_mcp_json(
    project_path: &str,
    binary: &std::path::Path,
    wavs_url: &str,
    token: Option<&str>,
) -> anyhow::Result<()> {
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME env var not set"))?;
    let claude_json = std::path::Path::new(&home).join(".claude.json");

    let mut config: serde_json::Value = if claude_json.exists() {
        let content = std::fs::read_to_string(&claude_json)?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let mut args = vec![
        serde_json::Value::String("--wavs-url".to_string()),
        serde_json::Value::String(wavs_url.to_string()),
    ];
    if let Some(t) = token {
        args.push(serde_json::Value::String("--token".to_string()));
        args.push(serde_json::Value::String(t.to_string()));
    }

    let entry = serde_json::json!({
        "command": binary.to_string_lossy(),
        "args": args,
    });

    // Upsert nested structure
    let obj = config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("~/.claude.json is not a JSON object"))?;
    obj.entry("projects").or_insert(serde_json::json!({}));
    config["projects"]
        .as_object_mut()
        .unwrap()
        .entry(project_path)
        .or_insert(serde_json::json!({}));
    config["projects"][project_path]
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert(serde_json::json!({}));

    config["projects"][project_path]["mcpServers"]["wavs"] = entry;

    // Write atomically via a temp file in the same directory
    let parent = claude_json.parent().unwrap();
    std::fs::create_dir_all(parent)?;
    let tmp_path = parent.join(format!(".claude.json.tmp.{}", std::process::id()));
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp_path)?;
        let json_str = serde_json::to_string_pretty(&config)?;
        f.write_all(json_str.as_bytes())?;
        f.write_all(b"\n")?;
    }
    std::fs::rename(&tmp_path, &claude_json)?;

    Ok(())
}

/// Read mcp_chain_credential and signing_mnemonic from a WAVS home wavs.toml.
/// Checks both the new `mcp_chain_credential` key and the legacy `chain_write_credential`
/// key so that projects that haven't yet been migrated still work.
fn read_wavs_home_credentials(wavs_home: &std::path::Path) -> (Option<String>, Option<String>) {
    let toml_path = wavs_home.join("wavs.toml");
    if !toml_path.exists() {
        return (None, None);
    }
    let content = match std::fs::read_to_string(&toml_path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(_) => return (None, None),
    };
    let wavs_section = match table.get("wavs").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return (None, None),
    };
    // Prefer new key, fall back to legacy key for migration compatibility
    let cred = wavs_section
        .get("mcp_chain_credential")
        .or_else(|| wavs_section.get("chain_write_credential"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mnem = wavs_section
        .get("signing_mnemonic")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (cred, mnem)
}

/// Write mcp_chain_credential and/or signing_mnemonic to ~/.wavs/wavs.toml.
/// Creates the directory and file if they don't exist. Upserts values in the
/// [wavs] section, preserving all other keys and sections.
fn write_global_wavs_credentials(
    mcp_chain_credential: Option<&str>,
    signing_mnemonic: Option<&str>,
) -> anyhow::Result<()> {
    if mcp_chain_credential.is_none() && signing_mnemonic.is_none() {
        return Ok(());
    }
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME env var not set"))?;
    let wavs_dir = std::path::Path::new(&home).join(".wavs");
    std::fs::create_dir_all(&wavs_dir)?;

    let toml_path = wavs_dir.join("wavs.toml");
    let mut table: toml::Table = if toml_path.exists() {
        let content = std::fs::read_to_string(&toml_path)?;
        content.parse().unwrap_or_default()
    } else {
        toml::Table::new()
    };

    {
        let wavs_section = table
            .entry("wavs")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let wavs_table = wavs_section
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("[wavs] is not a table in ~/.wavs/wavs.toml"))?;
        if let Some(cred) = mcp_chain_credential {
            wavs_table.insert(
                "mcp_chain_credential".to_string(),
                toml::Value::String(cred.to_string()),
            );
        }
        if let Some(mnem) = signing_mnemonic {
            wavs_table.insert(
                "signing_mnemonic".to_string(),
                toml::Value::String(mnem.to_string()),
            );
        }
    }

    let content = toml::to_string(&toml::Value::Table(table))
        .map_err(|e| anyhow::anyhow!("Failed to serialize TOML: {}", e))?;

    // Write atomically via a temp file
    let tmp_path = wavs_dir.join(format!(".wavs.toml.tmp.{}", std::process::id()));
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(content.as_bytes())?;
    }
    std::fs::rename(&tmp_path, &toml_path)?;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_register_claude_mcp(
    project_path: String,
    wavs_config: State<'_, WavsConfigState>,
    settings: State<'_, SettingsState>,
) -> AppResult<String> {
    let binary = find_mcp_binary().ok_or_else(|| {
        AppError::Service(
            "wavs-mcp binary not found. Build it with: cargo build -p wavs-mcp".to_string(),
        )
    })?;

    let wavs_url = match wavs_config.get_cloned() {
        Some(c) => format!("http://{}:{}", c.host, c.port),
        None => "http://localhost:8000".to_string(),
    };

    let s = settings.get_cloned();
    let token = s.mcp_token.as_deref();

    // Read credentials from the project wavs.toml
    let (cred, mnem) = s
        .wavs_home
        .as_deref()
        .map(read_wavs_home_credentials)
        .unwrap_or((None, None));

    // Write ~/.claude.json (command + args only, no credentials)
    register_claude_mcp_json(&project_path, &binary, &wavs_url, token)
        .map_err(|e| AppError::Io(e.to_string()))?;

    // Write credentials to ~/.wavs/wavs.toml (universal, all MCP clients)
    write_global_wavs_credentials(cred.as_deref(), mnem.as_deref())
        .map_err(|e| AppError::Io(e.to_string()))?;

    Ok(project_path)
}

// --- Storage (KV + Filesystem) ---

#[derive(Serialize, Deserialize)]
pub struct KvEntry {
    pub bucket: String,
    pub key: String,
    pub value_b64: String,
}

#[derive(Serialize, Deserialize)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_list_kv_entries(
    wavs_config: State<'_, WavsConfigState>,
    service_id: String,
) -> AppResult<Vec<KvEntry>> {
    let config = match wavs_config.get_cloned() {
        Some(cfg) => cfg,
        None => return Err(AppError::WavsConfig("WAVS config not loaded".to_string())),
    };
    let url = format!(
        "http://{}:{}/dev/kv/{}",
        config.host, config.port, service_id
    );
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AppError::Service(format!("Failed to fetch KV entries: {}", e)))?;
    if !response.status().is_success() {
        return Err(AppError::Service(format!(
            "KV list returned error: {}",
            response.status()
        )));
    }
    response
        .json::<Vec<KvEntry>>()
        .await
        .map_err(|e| AppError::Service(format!("Failed to parse KV entries: {}", e)))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_list_fs_entries(
    wavs_config: State<'_, WavsConfigState>,
    service_id: String,
    path: String,
) -> AppResult<Vec<FsEntry>> {
    let config = match wavs_config.get_cloned() {
        Some(cfg) => cfg,
        None => return Err(AppError::WavsConfig("WAVS config not loaded".to_string())),
    };
    let url = if path.is_empty() {
        format!(
            "http://{}:{}/dev/fs/{}",
            config.host, config.port, service_id
        )
    } else {
        format!(
            "http://{}:{}/dev/fs/{}/{}",
            config.host,
            config.port,
            service_id,
            path.trim_start_matches('/')
        )
    };
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AppError::Service(format!("Failed to fetch FS entries: {}", e)))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(vec![]);
    }
    if !response.status().is_success() {
        return Err(AppError::Service(format!(
            "FS list returned error: {}",
            response.status()
        )));
    }
    response
        .json::<Vec<FsEntry>>()
        .await
        .map_err(|e| AppError::Service(format!("Failed to parse FS entries: {}", e)))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_read_fs_file(
    wavs_config: State<'_, WavsConfigState>,
    service_id: String,
    path: String,
) -> AppResult<Vec<u8>> {
    let config = match wavs_config.get_cloned() {
        Some(cfg) => cfg,
        None => return Err(AppError::WavsConfig("WAVS config not loaded".to_string())),
    };
    let url = format!(
        "http://{}:{}/dev/fs/{}/{}",
        config.host,
        config.port,
        service_id,
        path.trim_start_matches('/')
    );
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| AppError::Service(format!("Failed to fetch file: {}", e)))?;
    if !response.status().is_success() {
        return Err(AppError::Service(format!(
            "File read returned error: {}",
            response.status()
        )));
    }
    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| AppError::Service(format!("Failed to read file bytes: {}", e)))
}

// --- Reset App State ---

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_clear_persisted_services(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    wavs_instance: State<'_, WavsInstanceState>,
) -> AppResult<()> {
    // Delete all live services from the running dispatcher (if available).
    // Errors on individual removes are logged but don't abort the reset.
    if let Ok(dispatcher) = wavs_instance.dispatcher() {
        match dispatcher
            .services
            .list(std::ops::Bound::Unbounded, std::ops::Bound::Unbounded)
        {
            Ok(services) => {
                for service in services {
                    let id = ServiceId::from(&service.manager);
                    if let Err(e) = dispatcher.remove_service(id) {
                        log::warn!("Failed to remove service during reset: {}", e);
                    }
                }
            }
            Err(e) => log::warn!("Failed to list services during reset: {}", e),
        }
    }

    // Clear all persisted state from settings.
    settings
        .update(&app, |s| {
            s.saved_service_managers.clear();
            s.saved_services.clear();
            s.saved_registries.clear();
        })
        .await?;
    log::info!("Cleared all persisted services and registries");
    Ok(())
}
