use crate::{
    pages::{
        logs::Logs, not_found::NotFound, services::Services, settings::SettingsUi,
        submissions::Submissions, triggers::Triggers,
    },
    prelude::*,
};

pub struct Body {
    state: AppState,
}

impl Body {
    pub fn new(state: AppState) -> Arc<Self> {
        Arc::new(Self { state })
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        let state = self.state.clone();

        static BODY_CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("height", "calc(100vh - 5rem)")
                .style("padding", "2rem")
                .style("overflow", "hidden")
            }
        });

        static PAGE_CONTENT: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "2rem")
                .style("border-radius", "0.5rem")
                .style("box-shadow", &format!("0 2px 8px {}", ColorRaw::Black15.value()))
                .style("border", &format!("1px solid {}", ColorRaw::CharcoalMedium.value()))
                .style("overflow", "hidden")
            }
        });

        html!("div", {
            .class([&*BODY_CONTAINER, ColorBackground::MainContent.class()])
            .child_signal(Route::signal().map(clone!(state => move |route| {
                Some(html!("div", {
                    .class([&*PAGE_CONTENT, FontSize::Lg.class(), ColorText::MainContent.class(), ColorBackground::NavBar.class()])
                    .child(match route {
                        Route::Logs => Logs::new(&state).render(),
                        Route::Settings => SettingsUi::new(state.clone()).render(),
                        Route::Services => Services::new(state.clone()).render(),
                        Route::Triggers => Triggers::new(state.clone()).render(),
                        Route::Submissions => Submissions::new(state.clone()).render(),
                        Route::NotFound => NotFound::new().render(),
                    })
                }))
            })))
        })
    }
}
