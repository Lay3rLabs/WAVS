pub mod commands;
pub mod listeners;
use anyhow::{anyhow, Context, Result};
use futures::future;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::config::CONFIG;

// Helper to access the thread-local global __TAURI__ safely with wasm-bindgen(thread_local_v2).
fn with_tauri<R>(f: impl FnOnce(&TauriInstance) -> R) -> R {
    TAURI_INSTANCE.with(|tauri| f(tauri))
}

fn core_api() -> TauriCoreApi {
    with_tauri(|t| t.core())
}

fn event_api() -> TauriEventApi {
    with_tauri(|t| t.event())
}

fn path_api() -> TauriPathApi {
    with_tauri(|t| t.path())
}

async fn invoke<RESP: DeserializeOwned>(fn_name: &str, args: &impl Serialize) -> Result<RESP> {
    // Set via Taskfile env, just for easier debugging
    if CONFIG.tauri_invoke_mock {
        future::pending().await
    } else {
        let args = serde_wasm_bindgen::to_value(args).map_err(|err| anyhow!("{:?}", err))?;
        match core_api().invoke(&format!("cmd_{fn_name}"), &args).await {
            Ok(data) => serde_wasm_bindgen::from_value(data).map_err(|err| anyhow!("{:?}", err)),
            Err(err) => Err(map_err(err)),
        }
    }
}

async fn invoke_no_resp(fn_name: &str, args: &impl Serialize) -> Result<()> {
    // Set via Taskfile env, just for easier debugging
    if CONFIG.tauri_invoke_mock {
        Ok(())
        //future::pending().await
    } else {
        let args = serde_wasm_bindgen::to_value(args).map_err(|err| anyhow!("{:?}", err))?;
        match core_api().invoke(&format!("cmd_{fn_name}"), &args).await {
            Ok(_) => Ok(()),
            Err(err) => Err(map_err(err)),
        }
    }
}

async fn invoke_no_args_no_resp(fn_name: &str) -> Result<()> {
    // Set via Taskfile env, just for easier debugging
    if CONFIG.tauri_invoke_mock {
        Ok(())
        //future::pending().await
    } else {
        match core_api()
            .invoke(&format!("cmd_{fn_name}"), &JsValue::null())
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(map_err(err)),
        }
    }
}

async fn invoke_no_args<RESP: DeserializeOwned>(fn_name: &str) -> Result<RESP> {
    // Set via Taskfile env, just for easier debugging
    if CONFIG.tauri_invoke_mock {
        future::pending().await
    } else {
        match core_api()
            .invoke(&format!("cmd_{fn_name}"), &JsValue::null())
            .await
        {
            Ok(data) => serde_wasm_bindgen::from_value(data).map_err(|err| anyhow!("{:?}", err)),
            Err(err) => Err(map_err(err)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct EventData<T> {
    event: String,
    payload: T,
}

async fn listen<F, T>(
    event_name: &str,
    mut callback: F,
) -> Result<Closure<dyn FnMut(wasm_bindgen::JsValue)>>
where
    F: FnMut(T) + 'static,
    T: DeserializeOwned,
{
    let callback = Closure::new(move |data: JsValue| {
        let data: Result<EventData<T>> =
            serde_wasm_bindgen::from_value(data).map_err(|err| anyhow!("{:?}", err));
        match data {
            Ok(data) => callback(data.payload),
            Err(err) => tracing::error!("{:?}", err),
        }
    });

    if !CONFIG.tauri_event_mock {
        event_api()
            .listen(event_name, &callback)
            .await
            .map_err(|err| anyhow!("{:?}", err))?;
    }

    Ok(callback)
}

async fn resolve_resource_path(path: &str) -> Result<String> {
    match path_api().resolve_resource(path).await {
        Ok(data) => data
            .as_string()
            .context(format!("{} is not a string", path)),
        Err(err) => Err(map_err(err)),
    }
}

pub async fn resource_img_url(path: &str) -> Result<String> {
    let res_path = resolve_resource_path(path).await?;

    let converted_path = core_api()
        .convert_file_src(&res_path)
        .as_string()
        .context("convert_file_src did not return a string")?;

    Ok(converted_path)
}

#[wasm_bindgen(js_namespace = ["window"])]
extern "C" {
    #[derive(Debug, Clone)]
    type TauriInstance;

    // wasm-bindgen deprecates imported JS statics unless they're marked thread-local.
    #[wasm_bindgen(js_name = "__TAURI__", thread_local_v2)]
    static TAURI_INSTANCE: TauriInstance;

    #[wasm_bindgen(getter, method)]
    fn core(this: &TauriInstance) -> TauriCoreApi;

    #[wasm_bindgen(getter, method)]
    fn event(this: &TauriInstance) -> TauriEventApi;

    #[wasm_bindgen(getter, method)]
    fn path(this: &TauriInstance) -> TauriPathApi;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone)]
    type TauriCoreApi;

    #[wasm_bindgen(catch, method)]
    async fn invoke(
        this: &TauriCoreApi,
        fn_name: &str,
        args: &JsValue,
    ) -> std::result::Result<JsValue, JsValue>;

    #[wasm_bindgen(method, js_name = "convertFileSrc")]
    fn convert_file_src(this: &TauriCoreApi, file_path: &str) -> JsValue;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone)]
    type TauriEventApi;

    #[wasm_bindgen(catch, method)]
    async fn listen(
        this: &TauriEventApi,
        event_name: &str,
        callback: &Closure<dyn FnMut(JsValue)>,
    ) -> std::result::Result<JsValue, JsValue>;
}

#[wasm_bindgen]
extern "C" {
    #[derive(Debug, Clone)]
    type TauriPathApi;

    #[wasm_bindgen(catch, method, js_name = "resolveResource")]
    async fn resolve_resource(
        this: &TauriPathApi,
        resource_path: &str,
    ) -> std::result::Result<JsValue, JsValue>;
}

fn map_err(err: JsValue) -> anyhow::Error {
    match err.as_string() {
        Some(err) => anyhow!(err),
        None => anyhow!("{:?}", err),
    }
}
