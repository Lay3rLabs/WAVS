use crate::prelude::*;

pub struct Header {
    state: AppState,
}

impl Header {
    pub fn new(state: AppState) -> Arc<Self> {
        Arc::new(Self { state })
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        static HEADER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "row")
                .style("align-items", "center")
                .style("justify-content", "space-between")
                .style("padding", "1rem 2rem")
                .style("border-bottom", &format!("1px solid {}", ColorRaw::CharcoalMedium.value()))
                .style("box-shadow", &format!("0 2px 8px {}", ColorRaw::Black15.value()))
            }
        });

        static LOGO_CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("align-items", "center")
            }
        });

        static NAVBAR: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "row")
                .style("align-items", "center")
                .style("gap", "0.5rem")
            }
        });

        html!("div", {
            .class([&*HEADER, ColorBackground::NavBar.class()])
            .child(html!("div", {
                .class(&*LOGO_CONTAINER)
                .fragment(&Image::new("wavs-logo.png")
                    .with_mixin(|dom| {
                        dom.style("height", "2.5rem")
                    })
                    .render())
            }))
            .child(html!("div", {
                .class([&*NAVBAR, FontSize::Lg.class()])
                .children([
                    Button::new()
                        .with_text("Services")
                        .with_disabled_signal(self.state.settings_complete_signal().map(|complete| !complete))
                        .with_selected_signal(Route::signal().map(|route| route == Route::Services))
                        .with_on_click(|| Route::Services.go_to_url())
                        .render(),
                    Button::new()
                        .with_text("Triggers")
                        .with_disabled_signal(self.state.settings_complete_signal().map(|complete| !complete))
                        .with_selected_signal(Route::signal().map(|route| route == Route::Triggers))
                        .with_on_click(|| Route::Triggers.go_to_url())
                        .render(),
                    Button::new()
                        .with_text("Submissions")
                        .with_disabled_signal(self.state.settings_complete_signal().map(|complete| !complete))
                        .with_selected_signal(Route::signal().map(|route| route == Route::Submissions))
                        .with_on_click(|| Route::Submissions.go_to_url())
                        .render(),
                    Button::new()
                        .with_text("Logs")
                        .with_disabled_signal(self.state.settings_complete_signal().map(|complete| !complete))
                        .with_selected_signal(Route::signal().map(|route| route == Route::Logs))
                        .with_on_click(|| Route::Logs.go_to_url())
                        .render(),
                    Button::new()
                        .with_link(Route::Settings.link())
                        .with_text("Settings")
                        .with_selected_signal(Route::signal().map(|route| route == Route::Settings))
                        .render(),
                ])
            }))
        })
    }
}
