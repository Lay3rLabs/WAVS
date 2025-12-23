use crate::prelude::*;

pub struct Services {
    address_input: Mutable<String>,
    chain_input: Mutable<String>,
    services: MutableVec<String>,
}

impl Services {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            address_input: Mutable::new(String::new()),
            chain_input: Mutable::new(String::new()),
            services: MutableVec::new(),
        })
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        let address_input = self.address_input.clone();
        let chain_input = self.chain_input.clone();
        let services = self.services.clone();

        static CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("gap", "2rem")
            }
        });

        static SECTION_TITLE: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("color", ColorRaw::BeigeLight.value())
                .style("font-size", "1.25rem")
                .style("font-weight", "600")
                .style("margin-bottom", "1rem")
            }
        });

        static ADD_SECTION: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "1.5rem")
                .style("border-radius", "0.5rem")
                .style("background-color", ColorRaw::CharcoalMedium.value())
                .style("border", &format!("1px solid {}", ColorRaw::CharcoalLight.value()))
            }
        });

        static INPUT_GRID: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "grid")
                .style("grid-template-columns", "1fr 1fr")
                .style("gap", "1rem")
                .style("margin-bottom", "1rem")
            }
        });

        static INPUT_GROUP: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("gap", "0.5rem")
            }
        });

        static INPUT_LABEL: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("color", ColorRaw::BeigeWarm.value())
                .style("font-size", "0.875rem")
                .style("font-weight", "500")
            }
        });

        static INPUT_FIELD: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "0.75rem 1rem")
                .style("border-radius", "0.375rem")
                .style("background-color", ColorRaw::CharcoalDark.value())
                .style("border", &format!("1px solid {}", ColorRaw::CharcoalLight.value()))
                .style("color", ColorRaw::BeigeWarm.value())
                .style("font-size", "0.875rem")
                .style("outline", "none")
                .style("transition", "border-color 0.2s")
            }
        });

        static SERVICES_LIST: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("gap", "0.75rem")
            }
        });

        static SERVICE_ITEM: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "1rem")
                .style("border-radius", "0.375rem")
                .style("background-color", ColorRaw::CharcoalMedium.value())
                .style("border", &format!("1px solid {}", ColorRaw::CharcoalLight.value()))
                .style("color", ColorRaw::BeigeWarm.value())
                .style("font-family", "monospace")
                .style("font-size", "0.875rem")
            }
        });

        static EMPTY_STATE: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "2rem")
                .style("text-align", "center")
                .style("color", ColorRaw::TanMuted.value())
                .style("font-style", "italic")
            }
        });

        html!("div", {
            .class(&*CONTAINER)
            // Add Service Section
            .child(html!("div", {
                .child(html!("div", {
                    .class(&*SECTION_TITLE)
                    .text("Add Service")
                }))
                .child(html!("div", {
                    .class(&*ADD_SECTION)
                    .child(html!("div", {
                        .class(&*INPUT_GRID)
                        .child(html!("div", {
                            .class(&*INPUT_GROUP)
                            .child(html!("label", {
                                .class(&*INPUT_LABEL)
                                .text("Address")
                            }))
                            .child(html!("input" => web_sys::HtmlInputElement, {
                                .class(&*INPUT_FIELD)
                                .attr("type", "text")
                                .attr("placeholder", "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2")
                                .prop_signal("value", address_input.signal_cloned())
                                .with_node!(element => {
                                    .event(clone!(address_input => move |_: events::Input| {
                                        let value = element.value();
                                        address_input.set(value);
                                    }))
                                })
                            }))
                        }))
                        .child(html!("div", {
                            .class(&*INPUT_GROUP)
                            .child(html!("label", {
                                .class(&*INPUT_LABEL)
                                .text("Chain")
                            }))
                            .child(html!("input" => web_sys::HtmlInputElement, {
                                .class(&*INPUT_FIELD)
                                .attr("type", "text")
                                .attr("placeholder", "e.g., ethereum")
                                .prop_signal("value", chain_input.signal_cloned())
                                .with_node!(element => {
                                    .event(clone!(chain_input => move |_: events::Input| {
                                        let value = element.value();
                                        chain_input.set(value);
                                    }))
                                })
                            }))
                        }))
                    }))
                    .child(
                        Button::new()
                            .with_text("Add Service")
                            .with_color(ButtonColor::Purple)
                            .with_on_click(clone!(address_input, chain_input, services => move || {
                                let address = address_input.get_cloned();
                                let chain = chain_input.get_cloned();
                                if !address.is_empty() && !chain.is_empty() {
                                    tracing::info!("Adding service: {} ({})", address, chain);
                                    services.lock_mut().push_cloned(format!("{} - {}", address, chain));
                                    address_input.set(String::new());
                                    chain_input.set(String::new());
                                    // TODO: Hook up to actual service addition
                                }
                            }))
                            .render()
                    )
                }))
            }))
            // Services List Section
            .child(html!("div", {
                .child(html!("div", {
                    .class(&*SECTION_TITLE)
                    .text("Active Services")
                }))
                .child_signal(services.signal_vec_cloned().is_empty().map(clone!(services => move |is_empty| {
                    if is_empty {
                        Some(html!("div", {
                            .class(&*EMPTY_STATE)
                            .text("No services configured yet")
                        }))
                    } else {
                        Some(html!("div", {
                            .class(&*SERVICES_LIST)
                            .children_signal_vec(services.signal_vec_cloned().map(move |service| {
                                html!("div", {
                                    .class(&*SERVICE_ITEM)
                                    .text(&service)
                                })
                            }))
                        }))
                    }
                })))
            }))
        })
    }
}
