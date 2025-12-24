use std::sync::Arc;

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use utils::{
    context::{AnyRuntime, AppContext},
    telemetry::{setup_metrics, Metrics},
};
use wavs::{config::HealthCheckMode, dispatcher::Dispatcher, health::SharedHealthStatus};
use wavs_gui_shared::{
    command::DirectoryChooserResponse,
    error::{AppError, AppResult},
    settings::Settings,
};
use wavs_types::{ChainConfigs, Service, ServiceManager};

use crate::state::{SettingsState, WavsConfigState, WavsInstance, WavsInstanceState};

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
pub async fn cmd_restart(app: AppHandle) {
    tauri::process::restart(&app.env());
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_start_wavs(
    app: AppHandle,
    wavs_config: State<'_, WavsConfigState>,
    wavs_instance: State<'_, WavsInstanceState>,
) -> AppResult<()> {
    let config = match wavs_config.get_cloned() {
        Some(cfg) => cfg,
        None => {
            return Err(AppError::WavsConfig("missing".to_string()));
        }
    };

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

    let dispatcher = Arc::new(Dispatcher::new_gui(&config, metrics.wavs, app).unwrap());

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
    wavs_instance: State<'_, WavsInstanceState>,
    manager: ServiceManager,
) -> AppResult<Service> {
    wavs_instance
        .dispatcher()?
        .add_service(manager)
        .await
        .map_err(|e| AppError::Service(format!("Failed to add service: {}", e)))
}
