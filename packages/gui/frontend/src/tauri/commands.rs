use std::path::PathBuf;

use crate::{config::CONFIG, tauri::invoke_no_args};
use anyhow::Result;
use wavs_gui_shared::{command::DirectoryChooserResponse, settings::Settings};

pub async fn set_wavs_home() -> Result<Option<PathBuf>> {
    let resp: DirectoryChooserResponse = invoke_no_args("set_wavs_home").await?;

    match resp {
        DirectoryChooserResponse::None => Ok(None),
        DirectoryChooserResponse::Selected(path) => Ok(Some(path)),
    }
}

pub async fn get_settings() -> Result<Settings> {
    if CONFIG.tauri_invoke_mock {
        return Ok(Settings::default());
    }

    invoke_no_args("get_settings").await
}

pub async fn restart() -> Result<Settings> {
    invoke_no_args("restart").await
}

pub async fn start_wavs() -> Result<()> {
    invoke_no_args("start_wavs").await
}
