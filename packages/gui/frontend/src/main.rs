#![allow(dead_code)]
#![allow(clippy::arc_with_non_send_sync)]
#![allow(clippy::type_complexity)]
#![allow(clippy::match_like_matches_macro)]
mod atoms;
mod body;
mod config;
mod header;
mod logger;
mod pages;
mod prelude;
mod route;
mod state;
mod tauri;
mod theme;
mod util;

use header::Header;
use prelude::*;

use crate::{body::Body, tauri::listeners::GlobalListeners};

pub fn main() {
    wasm_bindgen_futures::spawn_local(async {
        let state = AppState::new().await.unwrap_ext();

        logger::init_logger(state.clone());
        theme::stylesheet::init();

        GlobalListeners::start(state.clone()).await.unwrap_ext();

        if let Some(init_url) = CONFIG.debug.start_route.lock().unwrap_throw().take() {
            init_url.go_to_url();
        } else if !state.get_settings_complete() {
            Route::Settings.go_to_url();
        }

        dominator::append_dom(
            &dominator::body(),
            html!("div", {
                .future(clone!(state => async move {
                    if !state.get_settings_complete() {
                        return;
                    }

                    if let Err(err) = crate::tauri::commands::start_wavs().await {
                        Modal::open(move || {
                            html!("div", {
                                .style("padding", "1rem")
                                .child(html!("div", {
                                    .class([FontSize::Lg.class()])
                                    .text("Error Starting WAVS")
                                }))
                                .child(html!("p", {
                                    .class([FontSize::Md.class()])
                                    .text(&format!("There was an error starting WAVS: {}", err))
                                }))
                            })
                        });
                    }
                }))
                .child(Header::new(state.clone()).render())
                .child(Body::new(state.clone()).render())
                .fragment(&Modal::render())
            }),
        );
    });
}
