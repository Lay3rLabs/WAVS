use crate::prelude::*;

pub struct CloseX {}

impl CloseX {
    pub fn render(size: ButtonSize, on_click: impl Fn() + 'static) -> Dom {
        static CONTAINER_CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("cursor", "pointer")
                .style("border-radius", "50%")
            }
        });

        static SMALL_CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("width", "1rem")
                .style("height", "1rem")
            }
        });

        static LARGE_CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("width", "2rem")
                .style("height", "2rem")
            }
        });

        let hover = Mutable::new(false);

        html!("div", {
            .class(&*CONTAINER_CLASS)
            .class(match size {
                ButtonSize::Sm => &*SMALL_CLASS,
                _ => &*LARGE_CLASS,
            })
            .style_signal("background-color", hover.signal().map(|hover| match hover {
                true => ColorRaw::CharcoalLight.value(),
                false => ColorRaw::CharcoalMedium.value(),
            }))
            .child(svg!("svg", {
                .attrs!{
                    "viewBox": "0 0 40 40",
                    "fill": "none",
                    "xmlns": "http://www.w3.org/2000/svg",
                }
                .child(
                    svg!("path", {
                        .attr("d", "M 10,10 L 30,30 M 30,10 L 10,30")
                        .attr("stroke-width", "4")
                        .attr("stroke-linecap", "butt")
                        .attr_signal("stroke", hover.signal().map(|hover| match hover {
                            true => Some(ColorRaw::Whiteish.value()),
                            false => Some(ColorRaw::Whiteish.value()),
                        }))
                    })
                )
                .apply(set_on_hover(&hover))
                .event(move |_:events::Click| {
                    on_click();
                })
            }))
        })
    }
}
