use std::cell::RefCell;

use anyhow::Result;
use wasm_bindgen::prelude::*;
use wavs_gui_shared::event::{
    LogEvent, SettingsEvent, SubmissionEvent, TauriEventExt, TriggerEvent,
};

use crate::{logger::LogItem, state::AppState, tauri::listen};

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static GLOBAL_LISTENERS:RefCell<Option<GlobalListeners>> =  RefCell::new(None);
}

pub struct GlobalListeners {
    settings: Closure<dyn FnMut(JsValue)>,
    log: Closure<dyn FnMut(JsValue)>,
    triggers: Closure<dyn FnMut(JsValue)>,
    submissions: Closure<dyn FnMut(JsValue)>,
}

impl GlobalListeners {
    pub async fn start(state: AppState) -> Result<()> {
        let settings = start_settings_listener(state.clone()).await?;
        let log = start_log_listener(state.clone()).await?;
        let triggers = start_triggers_listener(state.clone()).await?;
        let submissions = start_submissions_listener(state.clone()).await?;

        GLOBAL_LISTENERS.with(|x| {
            *x.borrow_mut() = Some(Self {
                settings,
                log,
                triggers,
                submissions,
            })
        });

        Ok(())
    }
}

async fn start_settings_listener(state: AppState) -> Result<Closure<dyn FnMut(JsValue)>> {
    let callback = listen(SettingsEvent::NAME, move |evt: SettingsEvent| {
        state.settings.set(evt.settings);
    })
    .await?;

    Ok(callback)
}

async fn start_log_listener(state: AppState) -> Result<Closure<dyn FnMut(JsValue)>> {
    let callback = listen(LogEvent::NAME, move |evt: LogEvent| {
        // Parse level string to tracing::Level
        let level = match evt.level.to_uppercase().as_str() {
            "ERROR" => tracing::Level::ERROR,
            "WARN" => tracing::Level::WARN,
            "INFO" => tracing::Level::INFO,
            "DEBUG" => tracing::Level::DEBUG,
            "TRACE" => tracing::Level::TRACE,
            _ => tracing::Level::INFO,
        };

        // Get current timestamp
        let ts = {
            let millis = js_sys::Date::now();
            let secs = (millis / 1000.0) as u64;
            let nanos = ((millis % 1000.0) * 1_000_000.0) as u32;
            std::time::UNIX_EPOCH + std::time::Duration::new(secs, nanos)
        };

        state.log_list.lock_mut().push_cloned(LogItem {
            ts,
            level,
            target: evt.target,
            fields: evt.fields,
        });
    })
    .await?;

    Ok(callback)
}

async fn start_triggers_listener(state: AppState) -> Result<Closure<dyn FnMut(JsValue)>> {
    let callback = listen(TriggerEvent::NAME, move |evt: TriggerEvent| {
        state.triggers_list.lock_mut().push_cloned(evt.action);
    })
    .await?;

    Ok(callback)
}

async fn start_submissions_listener(state: AppState) -> Result<Closure<dyn FnMut(JsValue)>> {
    let callback = listen(SubmissionEvent::NAME, move |evt: SubmissionEvent| {
        state.submissions_list.lock_mut().push_cloned(evt);
    })
    .await?;

    Ok(callback)
}
