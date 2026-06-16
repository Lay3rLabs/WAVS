use serde::{Deserialize, Serialize};
use wavs_types::{ServiceId, TriggerAction, TriggerData, WorkflowId};

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
    pub trigger_data: TriggerData,
    pub tx_hash: Option<String>,
}

impl TauriEventExt for SubmissionEvent {
    const NAME: &'static str = "submission";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubmissionErrorEvent {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger_data: TriggerData,
    pub error_message: String,
}

impl TauriEventExt for SubmissionErrorEvent {
    const NAME: &'static str = "submission_error";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceEvent {
    pub action: ServiceAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Added,
    Removed,
    Paused,
    Resumed,
}

impl TauriEventExt for ServiceEvent {
    const NAME: &'static str = "service";
}
