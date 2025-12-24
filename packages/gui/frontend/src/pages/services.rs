use std::collections::BTreeMap;

use wasm_bindgen_futures::spawn_local;
use wavs_gui_shared::error::AppError;
use wavs_types::{AnyChainConfig, ChainKey, Service, ServiceId, ServiceManager};

use crate::prelude::*;

pub struct Services {
    address_input: Mutable<String>,
    chain_input: Mutable<Option<ChainKey>>,
    state: AppState,
}

impl Services {
    pub fn new(state: AppState) -> Arc<Self> {
        Arc::new(Self {
            address_input: Mutable::new(String::new()),
            chain_input: Mutable::new(None),
            state,
        })
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        let ready = Mutable::new(None);
        let state = &self.state;
        let _self = self.clone();

        #[derive(Clone, Debug)]
        struct ReadyInfo {
            chains: Vec<ChainKey>,
        }

        html!("div", {
            .child_signal(ready.signal_cloned().map(clone!(_self => move |info| {
                Some(match info {
                    None => {
                        html!("div", {
                            .class([FontSize::Lg.class(), ColorText::MainContent.class()])
                            .text("Loading services...")
                        })
                    }
                    Some(ReadyInfo { chains }) => {
                        _self.render_ready(chains)
                    }
                })
            })))
            .future(clone!(ready, state => async move {
                if let Err(err) = load_services(&state).await {
                    tracing::error!("Failed to load services: {}", err);
                    Modal::open_error_message(format!("Failed to load services: {}", err));
                    return;
                }

                // we shouldn't be able to reach this screen with invalid chain configs
                let chain_configs = crate::tauri::commands::get_chain_configs().await.unwrap_ext();

                ready.set(Some(ReadyInfo {
                    chains: chain_configs.all_chain_keys().unwrap_or_default()
                }));
            }))
        })
    }

    pub fn render_ready(self: &Arc<Self>, chains: Vec<ChainKey>) -> Dom {
        let address_input = self.address_input.clone();
        let chain_input = self.chain_input.clone();
        let state = &self.state;

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
                            .child(Dropdown::new()
                                .with_size(DropdownSize::Md)
                                .with_options(chains.into_iter().map(|chain_key| (chain_key.to_string(), chain_key)))
                                .with_on_change(clone!(chain_input => move |chain| {
                                    chain_input.set(Some(chain.clone()))
                                }))
                                .render()
                            )
                            // .child(html!("input" => web_sys::HtmlInputElement, {
                            //     .class(&*INPUT_FIELD)
                            //     .attr("type", "text")
                            //     .attr("placeholder", "e.g., ethereum")
                            //     .prop_signal("value", chain_input.signal_cloned())
                            //     .with_node!(element => {
                            //         .event(clone!(chain_input => move |_: events::Input| {
                            //             let value = element.value();
                            //             chain_input.set(value);
                            //         }))
                            //     })
                            // }))
                        }))
                    }))
                    .child(
                        Button::new()
                            .with_text("Add Service")
                            .with_color(ButtonColor::Purple)
                            .with_on_click(clone!(address_input, chain_input, state => move || {
                                let address = address_input.get_cloned();
                                let chain = chain_input.get_cloned();
                                if let Some(chain) = chain {
                                    if !address.is_empty() {
                                        address_input.set(String::new());
                                        chain_input.set(None);
                                        spawn_local(clone!(state => async move {
                                            if let Err(err) = add_service(&state, address, chain).await {
                                                tracing::error!("Failed to add service: {}", err);
                                                Modal::open_error_message(format!("Failed to add service: {}", err));
                                            }
                                        }));
                                    }
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
                .child_signal(state.services.signal_ref(|service_list| {
                    if service_list.is_empty() {
                        Some(html!("div", {
                            .class(&*EMPTY_STATE)
                            .text("No services configured yet")
                        }))
                    } else {
                        Some(html!("div", {
                            .class(&*SERVICES_LIST)
                            .children(service_list.values().map(|service| {
                                render_service_item(service)
                            }))
                        }))
                    }
                }))
            }))
        })
    }
}

fn render_service_item(service: &Service) -> Dom {
    let content = html!("pre", {
        .text(&serde_json::to_string_pretty(&service).unwrap_or_else(|_| "Failed to serialize service".to_string()))
    });

    render_expander(&service.name, content, false)
}

async fn add_service(state: &AppState, address: String, chain: ChainKey) -> anyhow::Result<()> {
    let chain_configs = crate::tauri::commands::get_chain_configs().await?;

    let config = match chain_configs.get_chain(&chain) {
        Some(config) => config,
        None => {
            return Err(AppError::MissingChain(chain).into());
        }
    };

    let manager = match config {
        AnyChainConfig::Cosmos(config) => ServiceManager::Cosmos {
            chain,
            address: config
                .to_chain_config()
                .parse_address(&address)?
                .try_into()?,
        },
        AnyChainConfig::Evm(_config) => ServiceManager::Evm {
            chain,
            address: address.parse()?,
        },
    };

    crate::tauri::commands::add_service(manager).await?;

    load_services(state).await?;

    Ok(())
}

async fn load_services(state: &AppState) -> anyhow::Result<()> {
    state.services.set(
        crate::tauri::commands::get_services()
            .await?
            .into_iter()
            .map(|s| (s.id(), s))
            .collect::<BTreeMap<ServiceId, Service>>(),
    );

    Ok(())
}
