use serde::{Deserialize, Serialize};

use crate::settings::Settings;

pub trait TauriEventExt: Serialize + Clone {
    const NAME: &'static str;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingsEvent {
    pub settings: Settings,
}

impl TauriEventExt for SettingsEvent {
    const NAME: &'static str = "settings";
}
