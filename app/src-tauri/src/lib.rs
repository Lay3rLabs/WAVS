// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::agent::PiSidecarState;
use crate::commands::{
    cmd_add_service, cmd_agent_abort, cmd_agent_get_messages, cmd_agent_new_session,
    cmd_agent_get_auth, cmd_agent_oauth_login, cmd_agent_prompt, cmd_agent_remove_auth,
    cmd_agent_respond_ui, cmd_agent_set_api_key, cmd_agent_set_model, cmd_agent_set_oauth,
    cmd_agent_set_thinking,
    cmd_agent_status, cmd_agent_list_sessions, cmd_agent_switch_session, cmd_save_agent_settings,
    cmd_clear_persisted_services, cmd_delete_mnemonic, cmd_get_chain_configs,
    cmd_get_component_digest, cmd_get_component_metadata, cmd_get_component_schema,
    cmd_get_health_status, cmd_get_mcp_binary_path, cmd_get_mcp_status,
    cmd_get_mnemonic, cmd_get_services, cmd_get_settings, cmd_get_wavs_url, cmd_has_mnemonic,
    cmd_list_fs_entries, cmd_list_kv_entries, cmd_pick_folder, cmd_publish_component,
    cmd_read_fs_file, cmd_read_wavs_toml, cmd_register_claude_mcp, cmd_remove_service, cmd_restart,
    cmd_save_env_vars, cmd_save_mcp_settings, cmd_save_poa_registries, cmd_save_service_to_node,
    cmd_set_wavs_home, cmd_start_agent, cmd_start_mcp_server, cmd_start_wavs, cmd_stop_agent,
    cmd_stop_mcp_server, cmd_store_mnemonic, cmd_upload_to_ipfs, cmd_write_wavs_toml,
};
use crate::state::{
    LogBufferState, McpServerState, MnemonicCacheState, SchemaCacheState, SettingsState,
    WavsConfigState, WavsInstanceState,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wavs::log_buffer::{InMemoryLogLayer, LogBufferInner};

mod agent;
mod commands;
mod logger;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = fix_path_env::fix();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Set up tracing subscriber to capture and forward to frontend, and into the
            // in-memory log buffer so agents can query structured logs via GET /dev/logs.
            let log_buffer = LogBufferInner::new();
            let tauri_log_layer = logger::TauriLogLayer::new(app.handle().clone());

            tracing_subscriber::registry()
                .with(tauri_log_layer)
                .with(InMemoryLogLayer::new(log_buffer.clone()))
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(std::io::stdout)
                        .with_target(true)
                        .with_level(true)
                        .compact(),
                )
                .with(tracing_subscriber::filter::LevelFilter::INFO)
                .init();

            let handle = app.handle();
            let settings_state = tauri::async_runtime::block_on(async move {
                SettingsState::load_or_new(handle).await.unwrap()
            });

            let wavs_home_path = { settings_state.inner.read().unwrap().wavs_home.clone() };
            let wavs_config_state = tauri::async_runtime::block_on(async move {
                match wavs_home_path {
                    Some(path) => WavsConfigState::load_or_default(path).await,
                    None => WavsConfigState::default(),
                }
            });

            // Log if wavs config couldn't be loaded (but don't clear wavs_home —
            // the directory is still valid even without a wavs.toml yet)
            if !wavs_config_state.is_set() {
                tracing::info!("No wavs config loaded (wavs.toml may not exist yet)");
            }

            app.manage(settings_state);
            app.manage(wavs_config_state);
            app.manage(WavsInstanceState::default());
            app.manage(MnemonicCacheState::default());
            app.manage(McpServerState::default());
            app.manage(SchemaCacheState::default());
            app.manage(PiSidecarState::default());
            app.manage(LogBufferState { inner: log_buffer });

            // Get primary monitor to calculate window size
            let monitors = app.primary_monitor()?;
            if let Some(monitor) = monitors {
                let size = monitor.size();
                let position = monitor.position();

                // Use 70% of screen width and height
                let width = size.width as f64 * 0.7;
                let height = size.height as f64 * 0.7;

                // Calculate centered position on primary monitor
                let x = position.x as f64 + ((size.width as f64 - width) / 2.0);
                let y = position.y as f64 + ((size.height as f64 - height) / 2.0);

                // Create window with correct size and position from the start
                WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                    .title("WAVS")
                    .inner_size(width, height)
                    .position(x, y)
                    .resizable(true)
                    .build()?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_set_wavs_home,
            cmd_pick_folder,
            cmd_get_settings,
            cmd_save_poa_registries,
            cmd_restart,
            cmd_start_wavs,
            cmd_get_chain_configs,
            cmd_add_service,
            cmd_remove_service,
            cmd_save_service_to_node,
            cmd_get_services,
            cmd_has_mnemonic,
            cmd_store_mnemonic,
            cmd_get_mnemonic,
            cmd_delete_mnemonic,
            cmd_get_health_status,
            cmd_read_wavs_toml,
            cmd_write_wavs_toml,
            cmd_upload_to_ipfs,
            cmd_get_component_digest,
            cmd_publish_component,
            cmd_get_component_schema,
            cmd_get_component_metadata,
            cmd_start_mcp_server,
            cmd_stop_mcp_server,
            cmd_get_mcp_status,
            cmd_get_mcp_binary_path,
            cmd_save_mcp_settings,
            cmd_clear_persisted_services,
            cmd_get_wavs_url,
            cmd_save_env_vars,
            cmd_register_claude_mcp,
            cmd_list_kv_entries,
            cmd_list_fs_entries,
            cmd_read_fs_file,
            cmd_start_agent,
            cmd_stop_agent,
            cmd_agent_prompt,
            cmd_agent_abort,
            cmd_agent_status,
            cmd_agent_new_session,
            cmd_agent_set_model,
            cmd_agent_set_thinking,
            cmd_agent_get_messages,
            cmd_agent_respond_ui,
            cmd_agent_get_auth,
            cmd_agent_oauth_login,
            cmd_agent_set_api_key,
            cmd_agent_set_oauth,
            cmd_agent_remove_auth,
            cmd_agent_list_sessions,
            cmd_agent_switch_session,
            cmd_save_agent_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
