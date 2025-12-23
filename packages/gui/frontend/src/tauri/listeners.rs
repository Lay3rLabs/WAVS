use std::cell::RefCell;

use anyhow::Result;
use wasm_bindgen::prelude::*;
use wavs_gui_shared::event::{SettingsEvent, TauriEventExt};

use crate::{state::AppState, tauri::listen};

thread_local! {
    static GLOBAL_LISTENERS:RefCell<Option<GlobalListeners>> = RefCell::new(None);
}

pub struct GlobalListeners {
    settings: Closure<dyn FnMut(JsValue)>,
}

impl GlobalListeners {
    pub async fn start(state: AppState) -> Result<()> {
        let settings = start_settings_listener(state.clone()).await?;

        GLOBAL_LISTENERS.with(|x| *x.borrow_mut() = Some(Self { settings }));

        Ok(())
    }
}

async fn start_settings_listener(state: AppState) -> Result<Closure<dyn FnMut(JsValue)>> {
    let callback = listen(SettingsEvent::NAME, move |evt: SettingsEvent| {
        state.settings_inner().set(evt.settings);
    })
    .await?;

    Ok(callback)
}
