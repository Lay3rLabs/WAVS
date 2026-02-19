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

use crate::state::{MnemonicCacheState, SettingsState, WavsConfigState, WavsInstance, WavsInstanceState};

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

    let dispatcher = Arc::new(Dispatcher::new(&config, metrics.wavs, app).unwrap());

    // Restore saved services from settings
    let saved_settings = settings.get_cloned();
    let saved_managers = saved_settings.saved_service_managers.clone();
    let saved_services = saved_settings.saved_services.clone();
    for manager in &saved_managers {
        match dispatcher.add_service(manager.clone()).await {
            Ok(_) => {}
            Err(e) => {
                log::warn!("Failed to restore saved service from chain: {}", e);
                // Fall back to the cached service definition to keep the service
                // registered in the dispatcher (so pause/resume work correctly).
                if let Some(cached) = saved_services.iter().find(|s| &s.manager == manager) {
                    match dispatcher.add_service_direct(cached.clone(), None).await {
                        Ok(_) => log::info!("Restored service from local cache: {:?}", manager),
                        Err(e2) => log::warn!("Failed to restore service from cache: {}", e2),
                    }
                }
            }
        }
    }

    let handle = std::thread::spawn({
        let ctx = ctx.clone();
        let dispatcher = dispatcher.clone();
        move || wavs::run_server(ctx, config, dispatcher, metrics.http, health_status)
    });

    wavs_instance.set(WavsInstance {
        ctx,
        meter_provider,
        handle,
        dispatcher,
    });

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
pub async fn cmd_pause_service(
    wavs_instance: State<'_, WavsInstanceState>,
    manager: ServiceManager,
) -> AppResult<()> {
    let service_id = ServiceId::from(&manager);
    wavs_instance
        .dispatcher()?
        .pause_service(service_id)
        .map_err(|e| AppError::Service(format!("Failed to pause service: {}", e)))?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_resume_service(
    wavs_instance: State<'_, WavsInstanceState>,
    manager: ServiceManager,
) -> AppResult<()> {
    let service_id = ServiceId::from(&manager);
    wavs_instance
        .dispatcher()?
        .resume_service(service_id)
        .map_err(|e| AppError::Service(format!("Failed to resume service: {}", e)))?;
    Ok(())
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
    let word_count = mnemonic.trim().split_whitespace().count();
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
pub async fn cmd_read_wavs_toml(
    settings: State<'_, SettingsState>,
) -> AppResult<String> {
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
            let part = reqwest::multipart::Part::bytes(content.into_bytes())
                .file_name("service.json");
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
            let part = reqwest::multipart::Part::bytes(content.into_bytes())
                .file_name(format!(
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

            let resp: PinataResponse = response
                .json()
                .await
                .map_err(|e| AppError::Service(format!("Failed to parse Pinata response: {}", e)))?;

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

    let package_ref: wasm_pkg_client::PackageRef = package
        .parse()
        .map_err(|e| AppError::Service(format!("Invalid package reference '{}': {}", package, e)))?;

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
