use crate::{logger::LogItem, prelude::*};
use std::time::UNIX_EPOCH;

pub struct Logs {
    list: MutableVec<LogItem>,
}

impl Logs {
    pub fn new(state: &AppState) -> Arc<Self> {
        Arc::new(Self {
            list: state.log_list.clone(),
        })
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        static SCROLL_CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("gap", "0.75rem")
                .style("max-height", "calc(100vh - 12rem)")
                .style("overflow-y", "auto")
                .style("padding-right", "0.5rem")
            }
        });

        html!("div", {
            .class(&*SCROLL_CONTAINER)
            .children_signal_vec(self.list.signal_vec_cloned().map(|item| {
                render_item(item)
            }))
        })
    }
}

fn render_item(item: LogItem) -> Dom {
    static CONTAINER_CLASS: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("border", &format!("1px solid {}", ColorRaw::CharcoalMedium.value()))
            .style("padding", "1rem")
            .style("border-radius", "0.5rem")
            .style("background-color", ColorRaw::CharcoalDark.value())
        }
    });

    static HEADER_CLASS: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("display", "flex")
            .style("gap", "1rem")
            .style("margin-bottom", "0.5rem")
            .style("align-items", "center")
        }
    });

    static LEVEL_CLASS: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("padding", "0.25rem 0.5rem")
            .style("border-radius", "0.25rem")
            .style("font-weight", "bold")
            .style("font-size", "0.875rem")
        }
    });

    static TIMESTAMP_CLASS: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("color", ColorRaw::TanMuted.value())
            .style("font-size", "0.875rem")
        }
    });

    static TARGET_CLASS: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("color", ColorRaw::TanWarm.value())
            .style("font-size", "0.875rem")
        }
    });

    static FIELDS_CLASS: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("margin-top", "0.5rem")
            .style("padding", "0.75rem")
            .style("background-color", ColorRaw::CharcoalDarkest.value())
            .style("border-radius", "0.25rem")
            .style("color", ColorRaw::BeigeLight.value())
            .style("font-family", "monospace")
            .style("font-size", "0.875rem")
            .style("overflow-x", "auto")
        }
    });

    // Format timestamp - simple approach showing elapsed time
    let timestamp = item
        .ts
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            let millis = d.subsec_millis();
            // Format as hours:minutes:seconds.milliseconds
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let secs = secs % 60;
            format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
        })
        .unwrap_or_else(|_| "Invalid".to_string());

    // Determine level color
    let level_bg_color = match item.level {
        tracing::Level::ERROR => "#dc2626",
        tracing::Level::WARN => "#f59e0b",
        tracing::Level::INFO => "#3b82f6",
        tracing::Level::DEBUG => "#8b5cf6",
        tracing::Level::TRACE => "#6b7280",
    };

    html!("div", {
        .class(&*CONTAINER_CLASS)
        .child(html!("div", {
            .class(&*HEADER_CLASS)
            .child(html!("span", {
                .class(&*LEVEL_CLASS)
                .style("background-color", level_bg_color)
                .style("color", "white")
                .text(&format!("{}", item.level))
            }))
            .child(html!("span", {
                .class(&*TIMESTAMP_CLASS)
                .text(&timestamp)
            }))
            .child(html!("span", {
                .class(&*TARGET_CLASS)
                .text(&item.target)
            }))
        }))
        .apply_if(!item.fields.is_empty(), |dom| {
            dom.child(html!("div", {
                .class(&*FIELDS_CLASS)
                .text(&item.fields)
            }))
        })
    })
}
