use tauri::{AppHandle, Emitter};
use wavs_gui_shared::{
    error::{AppError, AppResult},
    event::TauriEventExt,
};

pub trait EmitterExt {
    fn emit_ext<T: TauriEventExt>(&self, event: T) -> AppResult<()>;
}

impl EmitterExt for AppHandle {
    fn emit_ext<T: TauriEventExt>(&self, event: T) -> AppResult<()> {
        self.emit(T::NAME, event)
            .map_err(|err| AppError::EventEmitter(err.to_string()))
    }
}
