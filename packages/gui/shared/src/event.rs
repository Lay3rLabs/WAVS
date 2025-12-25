use serde::{Deserialize, Serialize};
use wavs_types::{Envelope, ServiceId, Submit, TriggerAction, TriggerData, WorkflowId};

use crate::error::AppResult;
use crate::settings::Settings;

pub trait TauriEventExt: Serialize + Clone {
    const NAME: &'static str;
}

pub trait TauriEventEmitterExt {
    fn emit_ext<T: TauriEventExt>(&self, event: T) -> AppResult<()>;
}

#[cfg(feature = "backend")]
impl<R: tauri::Runtime> TauriEventEmitterExt for tauri::AppHandle<R> {
    fn emit_ext<T: TauriEventExt>(&self, event: T) -> AppResult<()> {
        tauri::Emitter::emit(self, T::NAME, event)
            .map_err(|err| crate::error::AppError::EventEmitter(err.to_string()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingsEvent {
    pub settings: Settings,
}

impl TauriEventExt for SettingsEvent {
    const NAME: &'static str = "settings";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LogEvent {
    pub level: String,
    pub target: String,
    pub fields: String,
}

impl TauriEventExt for LogEvent {
    const NAME: &'static str = "log";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TriggerEvent {
    pub action: TriggerAction,
}

impl TauriEventExt for TriggerEvent {
    const NAME: &'static str = "trigger";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmissionEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub envelope: Envelope,
    pub trigger_data: TriggerData,
    pub submit: Submit,
}

impl TauriEventExt for SubmissionEvent {
    const NAME: &'static str = "submission";
}
