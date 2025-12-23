// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::commands::{cmd_get_settings, cmd_restart, cmd_set_wavs_home, cmd_start_wavs};
use crate::state::{SettingsState, WavsConfigState, WavsInstanceState};

mod commands;
mod event;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

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

            // normalize settings if wavs config is corrupted
            if !wavs_config_state.is_set() {
                settings_state.inner.write().unwrap().wavs_home = None;
            }

            app.manage(settings_state);
            app.manage(wavs_config_state);
            app.manage(WavsInstanceState::default());

            // Get primary monitor to calculate window size
            let monitors = app.primary_monitor()?;
            if let Some(monitor) = monitors {
                let size = monitor.size();
                let position = monitor.position();

                // Use 70% of screen width and height
                let width = (size.width as f64 * 0.7) as f64;
                let height = (size.height as f64 * 0.7) as f64;

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
            cmd_get_settings,
            cmd_restart,
            cmd_start_wavs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
