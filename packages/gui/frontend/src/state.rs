use std::collections::BTreeMap;

use futures_signals::{
    signal::{Mutable, Signal},
    signal_vec::MutableVec,
};
use wavs_gui_shared::{event::SubmissionEvent, settings::Settings};
use wavs_types::{Service, ServiceId, TriggerAction};

use crate::logger::LogItem;
// high-level app state shared by all components
#[derive(Clone)]
pub struct AppState {
    pub log_list: MutableVec<LogItem>,
    pub triggers_list: MutableVec<TriggerAction>,
    pub submissions_list: MutableVec<SubmissionEvent>,
    pub services: Mutable<BTreeMap<ServiceId, Service>>,
    pub settings: Mutable<Settings>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let settings = crate::tauri::commands::get_settings().await?;

        Ok(Self {
            log_list: MutableVec::new(),
            triggers_list: MutableVec::new(),
            submissions_list: MutableVec::new(),
            services: Mutable::new(BTreeMap::new()),
            settings: Mutable::new(settings),
        })
    }

    pub fn get_settings_complete(&self) -> bool {
        self.settings.lock_ref().wavs_home.is_some()
    }

    pub fn settings_complete_signal(&self) -> impl Signal<Item = bool> {
        self.settings.signal_ref(|s| s.wavs_home.is_some())
    }

    pub fn service_label(&self, service_id: &ServiceId) -> String {
        if let Some(service) = self.services.lock_ref().get(service_id) {
            service.name.clone()
        } else {
            "unknown".to_string()
        }
    }
}
