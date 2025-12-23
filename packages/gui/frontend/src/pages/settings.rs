use wasm_bindgen_futures::spawn_local;

use crate::{prelude::*, tauri};

pub struct SettingsUi {
    app_state: AppState,
    error_display: Mutable<Option<String>>,
    changed: Mutable<bool>,
}

impl SettingsUi {
    pub fn new(app_state: AppState) -> Arc<Self> {
        Arc::new(Self {
            app_state,
            error_display: Mutable::new(None),
            changed: Mutable::new(false),
        })
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        static HEADER_ROW: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("gap", "1rem")
                .style("margin-bottom", "2rem")
                .style("align-items", "center")
            }
        });

        static WARNING_BOX: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("flex", "1")
                .style("padding", "1rem")
                .style("border-radius", "0.5rem")
                .style("background-color", ColorRaw::CharcoalMedium.value())
                .style("border", &format!("1px solid {}", ColorRaw::CharcoalLight.value()))
            }
        });

        static WARNING_TEXT: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("color", ColorRaw::BeigeLight.value())
            }
        });

        static SETTINGS_SECTION: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("gap", "1rem")
            }
        });

        static SETTING_LABEL: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("color", ColorRaw::BeigeLight.value())
                .style("font-size", "1.125rem")
                .style("font-weight", "600")
                .style("margin-bottom", "0.5rem")
            }
        });

        static PICKER_ROW: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("gap", "0.75rem")
                .style("align-items", "center")
            }
        });

        static PATH_INPUT: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("flex", "1")
                .style("padding", "0.75rem 1rem")
                .style("border-radius", "0.375rem")
                .style("background-color", ColorRaw::CharcoalDark.value())
                .style("border", &format!("1px solid {}", ColorRaw::CharcoalLight.value()))
                .style("color", ColorRaw::BeigeWarm.value())
                .style("font-family", "monospace")
                .style("font-size", "0.875rem")
                .style("outline", "none")
                .style("transition", "border-color 0.2s")
            }
        });

        let app_state = &self.app_state;
        let error_display = &self.error_display;
        let changed = &self.changed;

        html!("div", {
            .child_signal(changed.signal().map(|changed| {
                if changed {
                    Some(html!("div", {
                        .class(&*HEADER_ROW)
                        .child(html!("div", {
                            .class(&*WARNING_BOX)
                            .child(html!("div", {
                                .class([FontSize::Lg.class(), &*WARNING_TEXT])
                                .text("⚠️ Restart for changes to take effect.")
                            }))
                        }))
                        .child(
                            Button::new()
                                .with_text("Restart Application")
                                .with_color(ButtonColor::Red)
                                .with_on_click(|| {
                                    spawn_local(async move {
                                        if let Err(err) = crate::tauri::commands::restart().await {
                                            tracing::error!("failed to restart application: {}", err);
                                        }
                                    });
                                })
                                .render()
                        )
                    }))
                } else {
                    None
                }
            }))
            .child(html!("div", {
                .class(&*SETTINGS_SECTION)
                .child(html!("div", {
                    .class(&*SETTING_LABEL)
                    .text("WAVS Home Directory")
                }))
                .child(html!("div", {
                    .class(&*PICKER_ROW)
                    .child(html!("input", {
                        .class(&*PATH_INPUT)
                        .attr("type", "text")
                        .attr("readonly", "true")
                        .attr("placeholder", "No directory selected")
                        .prop_signal("value", app_state.settings_inner().signal_ref(|settings| {
                            match &settings.wavs_home.as_ref() {
                                Some(path) => path.display().to_string(),
                                None => "".to_string(),
                            }
                        }))
                    }))
                    .child(
                        Button::new()
                            .with_text("Browse...")
                            .with_on_click(clone!(error_display, changed => move || {
                                error_display.set_neq(None);
                                spawn_local(clone!(error_display, changed => async move {
                                   match crate::tauri::commands::set_wavs_home().await {
                                       Ok(Some(home)) => {
                                           tracing::info!("changed wavs_home to {}", home.display());
                                           changed.set_neq(true);
                                       }
                                       Ok(None) => {
                                           // User cancelled the dialog, do nothing
                                       }
                                       Err(err) => {
                                           error_display.set_neq(Some(err.to_string()));
                                       }
                                   }
                                }));
                            }))
                            .render()
                    )
                }))
                .child_signal(error_display.signal_cloned().map(|error| {
                    error.map(|error| {
                        html!("div", {
                            .class([FontSize::Md.class(), ColorText::RedAlert.class()])
                            .text(&error)
                        })
                    })
                }))
            }))
        })
    }
}
