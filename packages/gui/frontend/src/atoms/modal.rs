use crate::prelude::*;

thread_local! {
    static MODAL:ModalInstance = ModalInstance::new()
}

struct ModalInstance {
    content: Mutable<Option<Box<dyn Fn() -> Dom>>>,
}

// Global API
#[allow(dead_code)]
pub struct Modal(ModalInstance);

impl Modal {
    pub fn open(content: impl Fn() -> Dom + 'static) {
        MODAL.with(|modal| {
            modal.open(content);
        });
    }

    pub fn open_error_message(msg: impl Into<String>) {
        static CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "2rem")
                .style("text-align", "center")
            }
        });

        let msg = msg.into();

        Self::open(move || {
            html!("div", {
                .class([&*CONTAINER, FontSize::Lg.class(), ColorText::RedAlert.class()])
                .text(&msg)
            })
        });
    }

    pub fn close() {
        MODAL.with(|modal| {
            modal.close();
        });
    }

    pub fn render() -> impl Fragment {
        MODAL.with(|modal| modal.render())
    }
}

impl ModalInstance {
    pub fn new() -> Self {
        Self {
            content: Mutable::new(None),
        }
    }

    pub fn open(&self, content: impl Fn() -> Dom + 'static) {
        self.content.set(Some(Box::new(content)));
    }

    pub fn close(&self) {
        self.content.set(None);
    }

    pub fn render(&self) -> impl Fragment {
        static BG: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("position", "fixed")
                .style("top", "0")
                .style("left", "0")
                .style("width", "100vw")
                .style("height", "100vh")
                .style("background", "rgba(0,0,0,0.5)")
            }
        });

        static CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("position", "fixed")
                .style("top", "50%")
                .style("left", "50%")
                .style("transform", "translate(-50%, -50%)")
                .style("background-color", "#fefefe")
                .style("border", "1px solid #888")
                .style("width", "80%")
                .style("height", "80%")
                .style("overflow", "auto")
            }
        });

        static CLOSE_BUTTON: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("position", "absolute")
                .style("top", ".5rem")
                .style("right", ".5rem")
            }
        });

        let m_content = self.content.clone();

        fragment!(move {
            .child_signal(m_content.signal_ref(clone!(m_content => move |content| {
                content.as_ref().map(|content| {
                    html!("div", {
                        .child(html!("div", {
                            .class(&*BG)
                            .event(clone!(m_content => move |_: events::Click| {
                                m_content.set(None);
                            }))
                        }))
                        .child(html!("div", {
                            .class(&*CONTAINER)
                            .child(html!("div", {
                                .class(&*CLOSE_BUTTON)
                                .child(CloseX::render(ButtonSize::Lg, clone!(m_content => move || {
                                    m_content.set(None);
                                })))
                            }))
                            .child(html!("div", {
                                .child(html!("div", {
                                    .style("padding", "20px")
                                    .child(content())
                                }))
                            }))
                        }))
                    })
                })
            })))
        })
    }
}
