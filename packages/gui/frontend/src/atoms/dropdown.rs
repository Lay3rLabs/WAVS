use crate::prelude::*;

pub struct Dropdown<T> {
    pub options: Vec<Arc<DropdownOption<T>>>,
    pub initial_selected: Option<T>,
    pub size: DropdownSize,
    pub on_change: Option<Arc<dyn Fn(&T)>>,
}

pub struct DropdownOption<T> {
    pub label: String,
    pub value: T,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropdownSize {
    Sm,
    Md,
}

impl DropdownSize {
    pub fn text_size_class(&self) -> &'static str {
        match self {
            Self::Sm => FontSize::Sm.class(),
            Self::Md => FontSize::Md.class(),
        }
    }

    pub fn container_class(&self) -> &'static str {
        static SM: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "0.5rem")
            }
        });

        static MD: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "0.5rem")
            }
        });
        match self {
            Self::Sm => &SM,
            Self::Md => &MD,
        }
    }

    pub fn options_class(&self) -> &'static str {
        static SM: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "0.5rem")
            }
        });

        static MD: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "1rem")
            }
        });
        match self {
            Self::Sm => &SM,
            Self::Md => &MD,
        }
    }
}

impl<T> Dropdown<T>
where
    T: PartialEq + 'static,
{
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            initial_selected: None,
            size: DropdownSize::Md,
            on_change: None,
        }
    }

    pub fn with_options(mut self, options: impl IntoIterator<Item = (String, T)>) -> Self {
        self.options = options
            .into_iter()
            .map(|(label, value)| Arc::new(DropdownOption { label, value }))
            .collect();
        self
    }

    pub fn with_intial_selected(mut self, initial_selected: Option<T>) -> Self {
        self.initial_selected = initial_selected;
        self
    }

    pub fn with_size(mut self, size: DropdownSize) -> Self {
        self.size = size;
        self
    }

    pub fn with_on_change(mut self, on_change: impl Fn(&T) + 'static) -> Self {
        self.on_change = Some(Arc::new(on_change));
        self
    }

    pub fn render(self) -> Dom {
        static CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "inline-flex")
                .style("flex-direction", "column")
                .style("gap", "1rem")
                .style(["user-select", "-webkit-user-select"], "none")
            }
        });

        static CONTENT: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "inline-block")
                .style("position", "relative")
                .style("border", "1px solid black")
                .style("border-radius", "4px")
                .style("cursor", "pointer")
            }
        });

        static LABEL_CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("gap", "1rem")
                .style("justify-content", "space-between")
                .style("padding", "1rem")
            }
        });

        static OPTIONS_CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("position", "absolute")
                .style("top", "100%")
                .style("left", "0")
                .style("width", "max-content")
                .style("z-index", Zindex::DropdownOptions.value())
            }
        });

        static OPTIONS_CONTENT: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("border", "1px solid black")
                .style("border-radius", "4px")
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("gap", "1rem")
                .style("max-height", "24rem")
                .style("overflow-y", "auto")
            }
        });

        let Self {
            options,
            initial_selected,
            size,
            on_change,
        } = self;

        let showing = Mutable::new(false);

        let selected: Mutable<Option<Arc<DropdownOption<T>>>> =
            Mutable::new(initial_selected.and_then(|initial_selected| {
                options
                    .iter()
                    .find(|option| option.value == initial_selected)
                    .cloned()
            }));

        let selected_label = selected.signal_cloned().map(|selected| {
            selected
                .map(|selected| selected.label.clone())
                .unwrap_or_else(|| "Select...".to_string())
        });

        html!("div", {
            .class(&*CONTAINER)
            .child(html!("div", {
                .class([&*CONTENT, ColorBackground::Dropdown.class()])
                .child(html!("div", {
                    .class([&*LABEL_CONTAINER, size.container_class(), ColorText::Dropdown.class()])
                    .child(html!("div", {
                        .class(size.text_size_class())
                        .text_signal(selected_label)
                    }))
                    .child(html!("div", {
                        .class(size.text_size_class())
                        .text_signal(showing.signal().map(|showing| {
                            if showing {
                                "▲"
                            } else {
                                "▼"
                            }
                        }))
                    }))
                    .event(clone!(showing => move |_: events::Click| {
                        showing.set(!showing.get());
                    }))
                }))
                .child_signal(showing.signal().map(clone!(on_change, showing => move |is_showing| {
                    if is_showing {
                        Some(html!("div", {
                            .class([&*OPTIONS_CONTAINER, ColorBackground::Dropdown.class()])
                            .child(html!("div", {
                                .class([&*OPTIONS_CONTENT, size.options_class()])
                                .children(options.iter().map(clone!(on_change, selected, showing => move |option| {
                                    let hovering = Mutable::new(false);
                                    html!("div", {
                                        .class(size.text_size_class())

                                        .text(&option.label)
                                        .style_signal("color", hovering.signal().map(|hovering| {
                                            if hovering {
                                                ColorText::DropdownHover.value()
                                            } else {
                                                ColorText::Dropdown.value()
                                            }
                                        }))
                                        .event({
                                            clone!(selected, option, showing, on_change => move |_: events::Click| {
                                                selected.set(Some(option.clone()));
                                                showing.set_neq(false);
                                                if let Some(on_change) = &on_change {
                                                    on_change(&option.value);
                                                }
                                            })
                                        })
                                        .apply(set_on_hover(&hovering))
                                    })
                                })))
                            }))
                        }))
                    } else {
                        None
                    }
                })))
                .with_node!(el => {
                    .global_event(clone!(showing => move |evt: events::Click| {
                        if let Some(target) = evt.target() {
                            if !el.contains(Some(target.unchecked_ref())) {
                                showing.set_neq(false);
                            }
                        }
                    }))
                })
            }))
        })
    }
}
