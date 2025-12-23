use futures_signals::{
    signal::{Mutable, Signal},
    signal_vec::MutableVec,
};
use wavs_gui_shared::settings::Settings;

use crate::logger::LogItem;
// high-level app state shared by all components
#[derive(Clone)]
pub struct AppState {
    pub log_list: MutableVec<LogItem>,
    _settings: Mutable<Settings>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let settings = crate::tauri::commands::get_settings().await?;

        Ok(Self {
            log_list: MutableVec::new(),
            _settings: Mutable::new(settings),
        })
    }

    pub fn get_settings_complete(&self) -> bool {
        self._settings.lock_ref().wavs_home.is_some()
    }

    pub fn settings_complete_signal(&self) -> impl Signal<Item = bool> {
        self._settings.signal_ref(|s| s.wavs_home.is_some())
    }

    pub fn settings_inner(&self) -> &Mutable<Settings> {
        &self._settings
    }
}
