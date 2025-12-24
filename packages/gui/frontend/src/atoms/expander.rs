use std::sync::LazyLock;

use dominator::{class, clone, html, Dom};
use futures_signals::signal::{Mutable, SignalExt};

use crate::theme::{color::ColorRaw, typography::FontSize};

pub fn render_expander(label: impl Into<String>, content: Dom, start_expanded: bool) -> Dom {
    let expanded = Mutable::new(start_expanded);
    let label = label.into();

    static CONTAINER: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("padding", "1rem")
            .style("border-radius", "0.375rem")
            .style("background-color", ColorRaw::CharcoalMedium.value())
            .style("border", &format!("1px solid {}", ColorRaw::CharcoalLight.value()))
            .style("color", ColorRaw::BeigeWarm.value())
        }
    });

    static LABEL: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("cursor", "pointer")
            .style(["user-select", "-webkit-user-select"], "none")
            .style("display", "flex")
            .style("align-items", "center")
            .style("gap", "0.5rem")
        }
    });

    static CONTENT: LazyLock<String> = LazyLock::new(|| {
        class! {
            .style("margin-top", "1rem")
            .style("padding", "1rem")
            .style("border-radius", "0.375rem")
            .style("background-color", ColorRaw::CharcoalMedium.value())
            .style("border", &format!("1px solid {}", ColorRaw::CharcoalLight.value()))
            .style("color", ColorRaw::BeigeWarm.value())
        }
    });

    html!("div", {
        .class([&*CONTAINER, FontSize::Md.class()])
        .child(html!("div", {
            .class(&*LABEL)
            .event(clone!(expanded => move |_: dominator::events::Click| {
                expanded.set(!expanded.get());
            }))
            .child(html!("div", {
                .text(&label)
            }))
            .child(html!("div", {
                .text_signal(expanded.signal().map(|showing| {
                    if showing {
                        "▼"
                    } else {
                        "▶"
                    }
                }))
            }))
        }))
        .child(html!("div", {
            .class(&*CONTENT)
            .style_signal("display", expanded.signal().map(|is_expanded| {
                if is_expanded {
                    "block"
                } else {
                    "none"
                }
            }))
            .child(content)
        }))

    })
}
